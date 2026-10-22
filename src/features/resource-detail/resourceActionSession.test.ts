import { describe, expect, test } from "bun:test";
import { QueryClient } from "@tanstack/svelte-query";
import { configureQueryCatalog, queryKeys } from "@/lib/queryKeys";
import { createMockTauriClient } from "@/lib/tauri-runtime";
import type {
	ResourceActionPreview,
	ResourceActionRequest,
	ResourceActionState,
	ResourceSummary,
} from "@/lib/types";
import {
	addEnvironment,
	removeEnvironment,
	removeQuantity,
	restoreQuantity,
	setImage,
	setReplicas,
} from "./resourceActionDrafts";
import { createResourceActionSession, type ResourceActionSessionState } from "./resourceActionSession";

// Failure modes (ADR 0024):
// 1. A state load, preview, or execution for a previous target or kubeconfig
//    source completes after the view moved on and shows, executes, or
//    overwrites state on the new target.
// 2. A mutation that completes after the target changed skips refreshing the
//    old target's details, YAML, revisions, and lists.
// 3. Execution runs without a ready preview: before one, while checking, after
//    a failed dry run, after a failed execution, or twice.
// 4. Execution omits the reviewed UID and resourceVersion guards, or a rollback
//    omits its revision source guards; a preview claims confirmation.
// 5. Drafts edited after the preview change what executes.
// 6. Delete skips review or typed-name confirmation when review skipping is on.
// 7. Skipping review hides a failed dry run or applies a no-op without showing it.
// 8. An independent action loses pending drafts or leaves the baseline stale.
// 9. An edit success clears drafts edited during its execution.
// 10. Removing a baseline value is dropped instead of sent, or a draft that
//     never reached the baseline is sent as a removal.
// 11. Denied or unconfirmed permissions still preview.
// 12. A closed review reopens or executes when its preview arrives.
// 13. A second review replaces an execution in flight and drops its result.
// 14. Edits ride along with a non-edit action.

const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

const deployment = (name = "payments-api"): ResourceSummary => ({
	cluster: "kind-dev",
	kind: "Deployment",
	apiVersion: "apps/v1",
	name,
	namespace: "default",
	age: "1d",
	health: "healthy",
});

function actionState(name: string, allowed = true): ResourceActionState {
	return {
		target: { clusterContext: "kind-dev", kind: "Deployment", name, namespace: "default" },
		uid: `${name}-uid`,
		resourceVersion: "7",
		replicas: 2,
		paused: false,
		containers: [
			{
				name: "api",
				image: "registry.example/api:v1",
				env: [{ name: "LOG_LEVEL", value: "info", referenced: false }],
				requests: { cpu: "100m" },
				limits: {},
			},
		],
		labels: {},
		annotations: {},
		permissions: (["edit", "restart", "pause", "resume", "rollback", "delete"] as const).map((action) => ({
			action,
			allowed,
			reason: "",
			requiredPermission: "patch deployments",
		})),
		revisions: [{ revision: 2, images: ["registry.example/api:v0"] }],
	};
}

function preview(request: ResourceActionRequest, changed = true): ResourceActionPreview {
	return {
		target: { clusterContext: request.clusterContext, kind: request.kind, name: request.name, namespace: request.namespace },
		effect: `${request.action} ${request.name}`,
		beforeYaml: "replicas: 2",
		afterYaml: changed ? "replicas: 3" : "replicas: 2",
		uid: `${request.name}-uid`,
		resourceVersion: "7",
		revisionUid: request.action === "rollback" ? "rs-uid" : undefined,
		revisionResourceVersion: request.action === "rollback" ? "41" : undefined,
	};
}

type Command = "get_resource_action_state" | "preview_resource_action" | "execute_resource_action";

interface Call {
	command: Command;
	request: ResourceActionRequest;
	resolve: () => void;
	reject: (message: string) => void;
}

function harness({
	skipReview = false,
	held = [],
	allowed = true,
	changed = true,
	failPreview = false,
}: {
	skipReview?: boolean;
	held?: Command[];
	allowed?: boolean;
	changed?: boolean;
	failPreview?: boolean;
} = {}) {
	const calls: Call[] = [];
	const respond = (command: Command, request: ResourceActionRequest) => {
		if (command === "get_resource_action_state") return actionState(request.name, allowed);
		if (command === "preview_resource_action") {
			if (failPreview) throw new Error("admission webhook denied the dry run");
			return preview(request, changed);
		}
		return { target: { clusterContext: request.clusterContext, kind: request.kind, name: request.name }, effect: `Applied ${request.action}` };
	};
	const handler = (command: Command) => (args: { request: ResourceActionRequest }) =>
		new Promise((resolve, reject) => {
			const call: Call = {
				command,
				request: args.request,
				resolve: () => {
					try {
						resolve(respond(command, args.request));
					} catch (cause) {
						reject(cause);
					}
				},
				reject: (message) => reject(new Error(message)),
			};
			calls.push(call);
			if (!held.includes(command)) call.resolve();
		});
	const client = createMockTauriClient({
		get_resource_action_state: handler("get_resource_action_state"),
		preview_resource_action: handler("preview_resource_action"),
		execute_resource_action: handler("execute_resource_action"),
	});
	const queryClient = new QueryClient();
	configureQueryCatalog(queryClient);
	let draftsApplied = 0;
	const session = createResourceActionSession({
		client,
		queryClient,
		skipReview: () => skipReview,
		onDraftsApplied: () => {
			draftsApplied += 1;
		},
	});
	const opened: boolean[] = [];
	session.subscribe((state: ResourceActionSessionState) => {
		if (state.review) opened.push(state.review.open);
	});
	return {
		session,
		queryClient,
		opened,
		draftsApplied: () => draftsApplied,
		calls: (command: Command) => calls.filter((call) => call.command === command),
		last: (command: Command) => calls.filter((call) => call.command === command).at(-1),
	};
}

async function ready(h: ReturnType<typeof harness>, resource = deployment(), kubeconfigSourceKey?: string) {
	h.session.retarget({ resource, kubeconfigSourceKey });
	await settle();
}

describe("resource action session: target changes", () => {
	test("drops a preview that completes after the target changed", async () => {
		const h = harness({ held: ["preview_resource_action"] });
		await ready(h);
		h.session.draft(setReplicas(3));
		void h.session.review("edit");
		await ready(h, deployment("checkout-api"));
		h.last("preview_resource_action")?.resolve();
		await settle();

		expect(h.session.current.review).toBe(null);
		expect(h.session.current.edits).toEqual({});
		expect(h.session.current.actionState?.uid).toBe("checkout-api-uid");
	});

	test("never executes a skipped review whose preview arrives after the target changed", async () => {
		const h = harness({ skipReview: true, held: ["preview_resource_action"] });
		await ready(h);
		void h.session.review("restart");
		await ready(h, deployment("checkout-api"));
		h.last("preview_resource_action")?.resolve();
		await settle();

		expect(h.calls("execute_resource_action")).toHaveLength(0);
		expect(h.session.current.inlineStatus).toBe(null);
	});

	test("refreshes the old target's reads when its execution completes after the target changed", async () => {
		const h = harness({ held: ["execute_resource_action"] });
		const old = deployment();
		const detailsKey = queryKeys.resourceDetails(old, undefined);
		h.queryClient.setQueryData(detailsKey, { name: old.name });
		await ready(h, old);
		await h.session.review("restart");
		void h.session.execute();
		await ready(h, deployment("checkout-api"));
		h.last("execute_resource_action")?.resolve();
		await settle();

		expect(h.queryClient.getQueryState(detailsKey)?.isInvalidated).toBe(true);
		expect(h.session.current.result).toBe(null);
		expect(h.session.current.actionState?.uid).toBe("checkout-api-uid");
	});

	test("keeps the newest state load when an older one completes last", async () => {
		const h = harness({ held: ["get_resource_action_state"] });
		h.session.retarget({ resource: deployment() });
		h.session.retarget({ resource: deployment("checkout-api") });
		h.calls("get_resource_action_state")[1]?.resolve();
		await settle();
		h.calls("get_resource_action_state")[0]?.resolve();
		await settle();

		expect(h.session.current.actionState?.uid).toBe("checkout-api-uid");
		expect(h.session.current.loading).toBe(false);
	});

	test("treats a kubeconfig source change as a new target", async () => {
		const h = harness();
		await ready(h, deployment(), "source-a");
		h.session.draft(setReplicas(3));

		expect(h.session.retarget({ resource: deployment(), kubeconfigSourceKey: "source-a" })).toBe(false);
		expect(h.session.current.edits).toEqual({ replicas: 3 });
		expect(h.session.retarget({ resource: deployment(), kubeconfigSourceKey: "source-b" })).toBe(true);
		expect(h.session.current.edits).toEqual({});
	});

	test("loads nothing for a kind outside the action allowlist", async () => {
		const h = harness();
		await ready(h, { ...deployment(), kind: "Secret", apiVersion: "v1" });

		expect(h.calls("get_resource_action_state")).toHaveLength(0);
		expect(h.session.current.loading).toBe(false);
	});
});

describe("resource action session: preview then execute", () => {
	test("executes the reviewed preview with its UID and resourceVersion guards", async () => {
		const h = harness();
		await ready(h);
		h.session.draft(setReplicas(3));
		await h.session.review("edit");
		await h.session.execute();

		expect(h.last("preview_resource_action")?.request.confirmed).toBe(false);
		expect(h.last("preview_resource_action")?.request.expectedUid).toBe(undefined);
		const executed = h.last("execute_resource_action")?.request;
		expect(executed?.confirmed).toBe(true);
		expect(executed?.expectedUid).toBe("payments-api-uid");
		expect(executed?.expectedResourceVersion).toBe("7");
	});

	test("rollback execution carries the reviewed revision source guards", async () => {
		const h = harness();
		await ready(h);
		await h.session.review("rollback", 2);
		await h.session.execute();

		const executed = h.last("execute_resource_action")?.request;
		expect(executed?.revision).toBe(2);
		expect(executed?.expectedRevisionUid).toBe("rs-uid");
		expect(executed?.expectedRevisionResourceVersion).toBe("41");
	});

	test("refuses to execute without a ready preview", async () => {
		const h = harness({ held: ["preview_resource_action", "execute_resource_action"] });
		await ready(h);
		await h.session.execute();
		void h.session.review("restart");
		await h.session.execute();
		h.last("preview_resource_action")?.resolve();
		await settle();
		void h.session.execute();
		void h.session.execute();

		expect(h.calls("execute_resource_action")).toHaveLength(1);
	});

	test("a failed dry run shows its error and cannot execute", async () => {
		const h = harness({ failPreview: true });
		await ready(h);
		await h.session.review("restart");
		await h.session.execute();

		expect(h.session.current.review?.status).toBe("failed");
		expect(h.session.current.review?.error).toContain("admission webhook");
		expect(h.calls("execute_resource_action")).toHaveLength(0);
	});

	test("a failed execution keeps its error and cannot be retried from the same preview", async () => {
		const h = harness({ held: ["execute_resource_action"] });
		await ready(h);
		await h.session.review("restart");
		void h.session.execute();
		h.last("execute_resource_action")?.reject("the object has been modified");
		await settle();
		await h.session.execute();

		expect(h.session.current.review?.executionAttempted).toBe(true);
		expect(h.session.current.review?.canExecute).toBe(false);
		expect(h.calls("execute_resource_action")).toHaveLength(1);
	});

	test("executes the reviewed drafts and keeps drafts edited during execution", async () => {
		const h = harness({ held: ["execute_resource_action"] });
		await ready(h);
		h.session.draft(setReplicas(3));
		await h.session.review("edit");
		void h.session.execute();
		h.session.draft(setReplicas(4));
		h.last("execute_resource_action")?.resolve();
		await settle();

		expect(h.last("execute_resource_action")?.request.edits).toEqual({ replicas: 3 });
		expect(h.session.current.edits).toEqual({ replicas: 4 });
		expect(h.draftsApplied()).toBe(0);
	});

	test("an applied edit clears the drafts it reviewed", async () => {
		const h = harness();
		await ready(h);
		h.session.draft(setReplicas(3));
		await h.session.review("edit");
		await h.session.execute();

		expect(h.session.current.edits).toEqual({});
		expect(h.session.current.result).toBe("Applied edit");
		expect(h.draftsApplied()).toBe(1);
	});

	test("does not start another review while an execution is in flight", async () => {
		const h = harness({ held: ["execute_resource_action"] });
		await ready(h);
		await h.session.review("restart");
		void h.session.execute();
		await h.session.review("pause");
		h.session.closeReview();
		h.last("execute_resource_action")?.resolve();
		await settle();

		expect(h.calls("preview_resource_action")).toHaveLength(1);
		expect(h.session.current.result).toBe("Applied restart");
	});

	test("a closed review ignores its late preview", async () => {
		const h = harness({ held: ["preview_resource_action"] });
		await ready(h);
		void h.session.review("restart");
		h.session.closeReview();
		h.last("preview_resource_action")?.resolve();
		await settle();

		expect(h.session.current.review).toBe(null);
	});
});

describe("resource action session: review skipping", () => {
	test("delete always opens review and needs the exact typed name", async () => {
		const h = harness({ skipReview: true });
		await ready(h);
		await h.session.review("delete");

		expect(h.session.current.review?.open).toBe(true);
		await h.session.execute();
		h.session.confirm("payments");
		await h.session.execute();
		expect(h.calls("execute_resource_action")).toHaveLength(0);

		h.session.confirm("payments-api");
		await h.session.execute();
		expect(h.last("execute_resource_action")?.request.action).toBe("delete");
	});

	test("applies a changed dry run inline without opening review", async () => {
		const h = harness({ skipReview: true });
		await ready(h);
		h.session.draft(setImage("api", "registry.example/api:v2"));
		await h.session.review("edit");

		expect(h.opened.includes(true)).toBe(false);
		expect(h.calls("execute_resource_action")).toHaveLength(1);
		expect(h.session.current.edits).toEqual({});
	});

	test("opens review with the error when a skipped dry run fails", async () => {
		const h = harness({ skipReview: true, failPreview: true });
		await ready(h);
		await h.session.review("restart");

		expect(h.session.current.review?.open).toBe(true);
		expect(h.session.current.review?.error).toContain("admission webhook");
		expect(h.calls("execute_resource_action")).toHaveLength(0);
	});

	test("opens review instead of applying a dry run that changes nothing", async () => {
		const h = harness({ skipReview: true, changed: false });
		await ready(h);
		await h.session.review("restart");

		expect(h.session.current.review?.open).toBe(true);
		expect(h.calls("execute_resource_action")).toHaveLength(0);
	});
});

describe("resource action session: drafts", () => {
	test("an independent action keeps drafts, sends none, and refreshes the baseline", async () => {
		const h = harness();
		await ready(h);
		h.session.draft(setImage("api", "registry.example/api:v2"));
		await h.session.review("restart");
		await h.session.execute();

		expect(h.last("execute_resource_action")?.request.edits).toBe(undefined);
		expect(h.calls("get_resource_action_state")).toHaveLength(2);
		await h.session.review("edit");
		expect(h.last("preview_resource_action")?.request.edits?.containers?.[0]?.image).toBe("registry.example/api:v2");
	});

	test("a review waits for the baseline reload after an independent action", async () => {
		const h = harness({ held: ["get_resource_action_state"] });
		await ready(h);
		h.last("get_resource_action_state")?.resolve();
		await settle();
		h.session.draft(setImage("api", "registry.example/api:v2"));
		await h.session.review("restart");
		const execution = h.session.execute();
		await settle();

		expect(h.session.current.loading).toBe(true);
		expect(h.session.draft(setImage("api", "registry.example/api:v3"))).toBe(false);
		void h.session.review("edit");
		await settle();
		expect(h.calls("preview_resource_action")).toHaveLength(1);

		h.last("get_resource_action_state")?.resolve();
		await execution;
		expect(h.session.current.loading).toBe(false);
		await h.session.review("edit");
		expect(h.calls("preview_resource_action")).toHaveLength(2);
		expect(h.last("preview_resource_action")?.request.edits?.containers?.[0]?.image).toBe("registry.example/api:v2");
	});

	test("removal sends only what the baseline has", async () => {
		const h = harness();
		await ready(h);
		h.session.draft(addEnvironment("api", "FEATURE_FLAG", "on"));
		h.session.draft(removeEnvironment("api", "FEATURE_FLAG"));
		h.session.draft(removeEnvironment("api", "LOG_LEVEL"));
		h.session.draft(removeQuantity("api", "requests", "cpu"));
		h.session.draft(removeQuantity("api", "limits", "memory"));
		await h.session.review("edit");

		const [container] = h.last("preview_resource_action")?.request.edits?.containers ?? [];
		expect(container?.env).toEqual([{ name: "LOG_LEVEL", remove: true }]);
		expect(container?.requests).toEqual({ cpu: null });
		expect(container?.limits).toEqual({});
	});

	test("refuses to add a variable the baseline defines and restores a removed quantity", async () => {
		const h = harness();
		await ready(h);

		expect(h.session.draft(addEnvironment("api", "LOG_LEVEL", "debug"))).toBe(false);
		h.session.draft(removeQuantity("api", "requests", "cpu"));
		h.session.draft(restoreQuantity("api", "requests", "cpu"));
		expect(h.session.current.edits.containers?.[0]?.requests).toEqual({});
	});

	test("denied or unconfirmed permissions never preview", async () => {
		const h = harness({ allowed: false, held: [] });
		h.session.retarget({ resource: deployment() });
		await h.session.review("restart");
		await settle();
		await h.session.review("restart");

		expect(h.calls("preview_resource_action")).toHaveLength(0);
		expect(h.session.draft(setReplicas(3))).toBe(true);
		await h.session.review("edit");
		expect(h.calls("preview_resource_action")).toHaveLength(0);
	});
});
