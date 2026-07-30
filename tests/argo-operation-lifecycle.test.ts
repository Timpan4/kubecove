import { describe, expect, test } from "bun:test";
import {
	ArgoOperationRefreshError,
	runArgoOperationLifecycle,
} from "../src/features/gitops/argo-operation-lifecycle";
import type { ArgoOperationRequest } from "../src/lib/gitops-types";

const request: ArgoOperationRequest = {
	transport: "connected",
	application: { name: "shop", namespace: "argocd" },
	action: "sync",
	resources: [],
};

const resolvedRequest: ArgoOperationRequest = { ...request, revision: "main" };

describe("Argo operation lifecycle", () => {
	test("rejects denied and malformed preflight", async () => {
		for (const result of [
			{ allowed: false, reason: "Denied" },
			{ allowed: true, preflightToken: "token" },
			{ allowed: true, resolvedRequest },
			{ allowed: true, preflightToken: "token", resolvedRequest: {} },
		]) {
			await expect(runArgoOperationLifecycle({
				request,
				preflight: async () => result,
				run: async () => ({ accepted: true }),
				refresh: async () => undefined,
			})).rejects.toThrow(result.reason ?? "Operation unavailable");
		}
	});

	test("runs resolved request then refreshes after acceptance", async () => {
		const events: string[] = [];
		let received: ArgoOperationRequest | undefined;
		await runArgoOperationLifecycle({
			request,
			preflight: async (value) => {
				expect(value).toBe(request);
				events.push("preflight");
				return { allowed: true, preflightToken: "token", resolvedRequest };
			},
			run: async (value) => {
				received = value;
				events.push("run");
				return { accepted: true };
			},
			refresh: async () => { events.push("refresh"); },
		});
		expect(received).toEqual({ ...resolvedRequest, preflightToken: "token" });
		expect(events).toEqual(["preflight", "run", "refresh"]);
	});

	test("distinguishes an accepted operation from a failed state refresh", async () => {
		let runs = 0;
		const lifecycle = runArgoOperationLifecycle({
			request,
			preflight: async () => ({ allowed: true, preflightToken: "token", resolvedRequest }),
			run: async () => {
				runs += 1;
				return { accepted: true };
			},
			refresh: async () => {
				throw new Error("Network unavailable");
			},
		});
		await expect(lifecycle).rejects.toBeInstanceOf(ArgoOperationRefreshError);
		expect(runs).toBe(1);
	});

	test("rejects backend refusal and permits retry", async () => {
		let attempts = 0;
		const invoke = () => runArgoOperationLifecycle({
			request,
			preflight: async () => ({ allowed: true, preflightToken: "token", resolvedRequest }),
			run: async () => ({ accepted: ++attempts > 1, message: "Not accepted" }),
			refresh: async () => undefined,
		});
		await expect(invoke()).rejects.toThrow("Not accepted");
		await expect(invoke()).resolves.toBeUndefined();
		expect(attempts).toBe(2);
	});
});
