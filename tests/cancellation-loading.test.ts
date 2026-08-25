import { describe, expect, test } from "bun:test";
import { shouldDropWarmupWatchEvent } from "../src/features/resources/helpers";
import {
	beginForegroundLoad,
	getForegroundLoadingSnapshot,
	withForegroundLoad,
} from "../src/lib/foreground-loading";
import {
	cancelBackendRequests,
	cancelWorkspaceRequests,
	listResourceMetrics,
	listResourceScope,
	listResourceTopology,
	type TauriClient,
} from "../src/lib/tauri";
import type {
	ResourceListRequest,
	ResourceMetricsSummary,
	ResourceTopology,
	JsonObject,
	JsonValue,
} from "../src/lib/types";

function mockInvokeResult<T>(value: JsonValue): T {
	// SAFETY: test mock provides JSON-shaped responses for exact wrapper results asserted below.
	return value as T;
}

describe("cancellable resource loads", () => {
	test("passes cancellable metadata through resource wrappers", async () => {
		const calls: Array<{ cmd: string; args?: JsonObject }> = [];
		const metrics: ResourceMetricsSummary = {
			cluster: "kind-dev",
			availability: { status: "available", message: "metrics available" },
			pods: [],
			nodes: [],
			workloads: [],
			warnings: [],
		};
		const topology: ResourceTopology = { nodes: [], edges: [], warnings: [] };
		const client: TauriClient = {
			invoke: async <T>(cmd: string, args?: JsonObject) => {
				calls.push({ cmd, args });
				if (cmd === "list_resource_metrics") return mockInvokeResult<T>(metrics);
				if (cmd === "list_resource_topology") return mockInvokeResult<T>(topology);
				if (cmd === "cancel_backend_requests") return mockInvokeResult<T>({ cancelled: 2 });
				if (cmd === "cancel_workspace_requests") {
					return mockInvokeResult<T>({
						cancelledRequests: 3,
						cancelledLoads: 1,
						clientGeneration: 7,
					});
				}
				return mockInvokeResult<T>([]);
			},
		};
		const cancellable = {
			cancelScope: "resources:scope-1",
			requestId: "resources-request-1",
		};
		const requests: ResourceListRequest[] = [{ kind: "Pod", namespace: "default" }];

		await listResourceScope(client, "kind-dev", requests, "KUBECONFIG", cancellable);
		await listResourceTopology(
			client,
			"kind-dev",
			["default"],
			"owner",
			"KUBECONFIG",
			cancellable,
		);
		await listResourceMetrics(client, "kind-dev", ["default"], "KUBECONFIG", cancellable);
		expect(await cancelBackendRequests(client, "resources:scope-1")).toEqual({
			cancelled: 2,
		});
		expect(await cancelWorkspaceRequests(client)).toEqual({
			cancelledRequests: 3,
			cancelledLoads: 1,
			clientGeneration: 7,
		});

		expect(calls).toEqual([
			{
				cmd: "list_resource_scope",
				args: {
					clusterContext: "kind-dev",
					requests,
					kubeconfigEnvVar: "KUBECONFIG",
					cancelScope: "resources:scope-1",
					requestId: "resources-request-1",
				},
			},
			{
				cmd: "list_resource_topology",
				args: {
					clusterContext: "kind-dev",
					namespaces: ["default"],
					mode: "owner",
					kubeconfigEnvVar: "KUBECONFIG",
					cancelScope: "resources:scope-1",
					requestId: "resources-request-1",
				},
			},
			{
				cmd: "list_resource_metrics",
				args: {
					clusterContext: "kind-dev",
					namespaces: ["default"],
					kubeconfigEnvVar: "KUBECONFIG",
					cancelScope: "resources:scope-1",
					requestId: "resources-request-1",
				},
			},
			{
				cmd: "cancel_backend_requests",
				args: { cancelScope: "resources:scope-1" },
			},
			{
				cmd: "cancel_workspace_requests",
				args: undefined,
			},
		]);
	});
});

describe("foreground loading", () => {
	test("tracks foreground work until completion", async () => {
		expect(getForegroundLoadingSnapshot()).toBe(0);

		const done = beginForegroundLoad("resources");
		expect(getForegroundLoadingSnapshot()).toBe(1);
		done();
		expect(getForegroundLoadingSnapshot()).toBe(0);

		const value = await withForegroundLoad("details", async () => {
			expect(getForegroundLoadingSnapshot()).toBe(1);
			return "ok";
		});

		expect(value).toBe("ok");
		expect(getForegroundLoadingSnapshot()).toBe(0);
	});
});

describe("resource watch warmup", () => {
	test("drops initial added events but allows real changes", () => {
		expect(shouldDropWarmupWatchEvent("added", 1999)).toBe(true);
		expect(shouldDropWarmupWatchEvent("added", 2000)).toBe(false);
		expect(shouldDropWarmupWatchEvent("modified", 50)).toBe(false);
		expect(shouldDropWarmupWatchEvent("deleted", 50)).toBe(false);
	});
});
