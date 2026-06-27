import { describe, expect, test } from "bun:test";
import { shouldDropWarmupWatchEvent } from "../src/features/resources/helpers";
import {
	createCancellableRequest,
	createCancelScope,
} from "../src/lib/cancellable-loads";
import {
	beginForegroundLoad,
	getForegroundLoadingSnapshot,
	withForegroundLoad,
} from "../src/lib/foreground-loading";
import {
	cancelBackendRequests,
	listResourceMetrics,
	listResourceScope,
	listResourceTopology,
	type TauriClient,
} from "../src/lib/tauri";
import type {
	ResourceListRequest,
	ResourceMetricsSummary,
	ResourceTopology,
} from "../src/lib/types";

describe("cancellable resource loads", () => {
	test("creates stable scope strings and unique request ids", () => {
		const parts = ["kind-dev", ["default"], ["Pod"]];
		const scope = createCancelScope("resources", parts);
		const first = createCancellableRequest(scope, "resources");
		const second = createCancellableRequest(scope, "resources");

		expect(scope).toBe('resources:["kind-dev",["default"],["Pod"]]');
		expect(first.cancelScope).toBe(scope);
		expect(second.cancelScope).toBe(scope);
		expect(first.requestId).not.toBe(second.requestId);
		expect(first.requestId.startsWith("resources-")).toBe(true);
	});

	test("passes cancellable metadata through resource wrappers", async () => {
		const calls: Array<{ cmd: string; args?: Record<string, unknown> }> = [];
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
			invoke: async <T>(cmd: string, args?: Record<string, unknown>) => {
				calls.push({ cmd, args });
				if (cmd === "list_resource_metrics") return metrics as T;
				if (cmd === "list_resource_topology") return topology as T;
				if (cmd === "cancel_backend_requests") return { cancelled: 2 } as T;
				return [] as T;
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
