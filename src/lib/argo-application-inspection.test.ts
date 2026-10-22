import { afterEach, describe, expect, test } from "bun:test";
import { QueryClient } from "@tanstack/svelte-query";
import { createArgoApplicationInspection, type ArgoInspectionInput } from "./argo-application-inspection";
import type { ArgoConnectionPreference } from "./argo-connection-policy";
import type { ArgoApplicationInspector, ArgoConnectionStatus } from "./gitops-types";
import { configureQueryCatalog, queryKeys } from "./queryKeys";
import type { SavedArgoProfile } from "./settings";
import { createMockTauriClient } from "./tauri-runtime";

// Failure modes: Automatic inspection starts before profile statuses arrive or
// ignores saved order; an explicit Kubernetes preference waits on statuses; an
// unavailable explicit profile is replaced by another; a profile from another
// workspace is used; one failed status read hides the healthy ones; a Kubernetes
// fallback hides its Connected failure; unredacted data outlives its view; a
// hidden view reads anyway.

const settle = () => new Promise((resolve) => setTimeout(resolve, 10));
const cleanups: (() => void)[] = [];

afterEach(() => {
	for (const cleanup of cleanups.splice(0)) cleanup();
});

function profile(id: string, workspaceId = "workspace-1"): SavedArgoProfile {
	return {
		id,
		endpoint: { kind: "externalHttps", url: `https://${id}.example.com` },
		clusterContext: "kind-dev",
		workspaceId,
		kubeconfigSourceKey: "KUBECONFIG",
		rememberCredential: false,
	};
}

function status(connected: boolean): ArgoConnectionStatus {
	return {
		profile: null,
		connected,
		username: null,
		unavailableReason: connected ? null : "Session expired",
	};
}

function inspector(transport: "connected" | "kubernetes", patch: Partial<ArgoApplicationInspector> = {}): ArgoApplicationInspector {
	return {
		application: { name: "guestbook", namespace: "argocd", uid: "uid-1" },
		status: null,
		history: [],
		resources: [],
		comparisons: [],
		conditions: [],
		operationState: null,
		connected: transport === "connected",
		transport,
		provenance: transport,
		...patch,
	};
}

interface InspectorArgs {
	transport: string;
	connectionId?: string;
	application: { name: string; context?: string | null; workspaceId?: string | null; apiVersion?: string | null };
}

function harness({
	statuses = {},
	result = () => inspector("kubernetes"),
}: {
	statuses?: Record<string, ArgoConnectionStatus | Error | Promise<ArgoConnectionStatus>>;
	result?: (args: InspectorArgs) => ArgoApplicationInspector;
}) {
	const queryClient = new QueryClient();
	configureQueryCatalog(queryClient);
	const statusReads: string[] = [];
	const inspections: InspectorArgs[] = [];
	const client = createMockTauriClient({
		get_argo_connection_status: async ({ id }: { id: string }) => {
			statusReads.push(id);
			const response = statuses[id];
			if (!response || response instanceof Error) throw response ?? new Error("unknown profile");
			return response;
		},
		get_argo_application_inspector: (args: InspectorArgs) => {
			inspections.push(args);
			return result(args);
		},
	});
	const inspection = createArgoApplicationInspection(client, queryClient);
	const view = (
		patch: Partial<ArgoInspectionInput> = {},
		preference: ArgoConnectionPreference = { kind: "automatic" },
		profiles = [profile("primary"), profile("backup")],
	) => {
		inspection.update({
			readContext: {
				workspaceId: "workspace-1",
				clusterContext: "kind-dev",
				namespaceScope: [],
				kubeconfigSourceKey: "KUBECONFIG",
				sourceReady: true,
				sourceError: null,
				showKubeconfigSourceLabels: false,
			},
			application: { name: "guestbook", namespace: "argocd", uid: "uid-1" },
			settings: {
				argoProfiles: profiles,
				argoConnectionPreferences: { "workspace-1": preference },
				redactSecrets: true,
			},
			...patch,
		});
	};
	const observe = () => {
		const stop = inspection.subscribe(() => {});
		cleanups.push(stop);
		return stop;
	};
	cleanups.push(() => queryClient.clear());
	return { inspection, queryClient, statusReads, inspections, view, observe };
}

describe("Argo Application inspection", () => {
	test("Automatic waits for profile statuses, then uses the first healthy profile in saved order", async () => {
		let resolveBackup!: (value: ArgoConnectionStatus) => void;
		const run = harness({
			statuses: {
				primary: status(false),
				backup: new Promise((resolve) => { resolveBackup = resolve; }),
				other: status(true),
			},
			result: () => inspector("connected"),
		});
		run.view({}, { kind: "automatic" }, [profile("primary"), profile("other", "workspace-2"), profile("backup")]);
		run.observe();
		await settle();

		expect(run.inspection.current).toMatchObject({ checkingProfiles: true, ready: false, loading: true });
		expect(run.statusReads).toEqual(["primary", "backup"]);
		expect(run.inspections).toEqual([]);

		resolveBackup(status(true));
		await settle();

		expect(run.inspections).toMatchObject([
			{ transport: "connected", connectionId: "backup", application: { name: "guestbook", context: "kind-dev", workspaceId: "workspace-1", apiVersion: "argoproj.io/v1alpha1" } },
		]);
		expect(run.inspection.current).toMatchObject({
			checkingProfiles: false,
			ready: true,
			loading: false,
			choice: { transport: "connected", connectionId: "backup" },
			selectedStatus: { connected: true },
			data: { transport: "connected" },
		});
	});

	test("an explicit Kubernetes preference inspects without waiting for profile statuses", async () => {
		const run = harness({ statuses: { primary: new Promise(() => {}), backup: new Promise(() => {}) } });
		run.view({}, { kind: "kubernetes" });
		run.observe();
		await settle();

		expect(run.inspections).toMatchObject([{ transport: "kubernetes" }]);
		expect(run.inspections[0]?.connectionId).toBeUndefined();
		expect(run.inspection.current).toMatchObject({ ready: true, choice: { preference: { kind: "kubernetes" } }, data: { transport: "kubernetes" } });
	});

	test("an unavailable explicit Connected profile is never replaced by another profile", async () => {
		const run = harness({ statuses: { primary: status(true), backup: status(false) } });
		run.view({}, { kind: "connected", profileId: "backup" });
		run.observe();
		await settle();

		expect(run.inspections).toEqual([]);
		expect(run.inspection.current).toMatchObject({
			ready: false,
			loading: false,
			choice: { transport: "connected", connectionId: "backup", unavailable: true },
			selectedStatus: { unavailableReason: "Session expired" },
		});
	});

	test("a failed status read leaves the other profiles usable", async () => {
		const run = harness({ statuses: { primary: new Error("offline"), backup: status(true) }, result: () => inspector("connected") });
		run.view();
		run.observe();
		await settle();

		expect(run.inspections).toMatchObject([{ transport: "connected", connectionId: "backup" }]);
	});

	test("a Kubernetes fallback stays visible with its Connected failure", async () => {
		const fallback = inspector("kubernetes", {
			connectedFallback: { transport: "connected", failure: { kind: "unavailable", message: "Argo CD server unreachable" } },
		});
		const run = harness({ statuses: { primary: status(true), backup: status(true) }, result: () => fallback });
		run.view();
		run.observe();
		await settle();

		expect(run.inspection.current).toMatchObject({
			choice: { transport: "connected", connectionId: "primary" },
			error: null,
			data: { transport: "kubernetes", connectedFallback: { failure: { message: "Argo CD server unreachable" } } },
		});
	});

	test("a hidden view or a view without an Application reads nothing", async () => {
		const run = harness({ statuses: { primary: status(true), backup: status(true) } });
		run.view({ enabled: false });
		run.observe();
		await settle();
		run.view({ application: null });
		await settle();

		expect(run.statusReads).toEqual([]);
		expect(run.inspections).toEqual([]);
		expect(run.inspection.current).toMatchObject({ ready: false, loading: false, request: null, data: undefined });
		await run.inspection.refresh();
		expect(run.inspections).toEqual([]);
	});

	test("refresh reads the inspection again through the same transport", async () => {
		const run = harness({});
		run.view({}, { kind: "kubernetes" });
		run.observe();
		await settle();
		await run.inspection.refresh();

		expect(run.inspections).toHaveLength(2);
		expect(run.inspections.map((args) => args.transport)).toEqual(["kubernetes", "kubernetes"]);
	});

	test.each([true, false])("an inspection stays cached after its view stops only when redacted (redactSecrets: %p)", async (redactSecrets) => {
		const run = harness({});
		const settings = { argoProfiles: [], argoConnectionPreferences: { "workspace-1": { kind: "kubernetes" as const } }, redactSecrets };
		run.view({ settings });
		const stop = run.observe();
		await settle();
		stop();
		await settle();

		const applicationReads = queryKeys.argoWorkspaceApplicationScope("kind-dev", "workspace-1", "guestbook", "argocd", "KUBECONFIG");
		expect(run.queryClient.getQueryCache().findAll({ queryKey: applicationReads })).toHaveLength(redactSecrets ? 1 : 0);
	});
});
