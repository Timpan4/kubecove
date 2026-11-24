import { describe, expect, test } from "bun:test";
import {
	formatCompactResourceMetrics,
	formatCpuMillicores,
	formatMemoryBytes,
	resourceMetricFor,
	metricsCard,
	resourceMetricIndex,
} from "../src/lib/resource-metrics";
import type {
	ResourceMetricsSummary,
	ResourceSummary,
} from "../src/lib/types";

const metrics: ResourceMetricsSummary = {
	cluster: "kind-dev",
	availability: { status: "available", message: "metrics available" },
	pods: [
		{
			kind: "Pod",
			cluster: "kind-dev",
			name: "api-0",
			namespace: "payments",
			cpuMillicores: 125,
			memoryBytes: 64 * 1024 * 1024,
			sampledAt: "2026-05-22T12:00:00Z",
			sourcePods: [],
		},
	],
	nodes: [],
	workloads: [
		{
			kind: "ReplicaSet",
			cluster: "kind-dev",
			name: "api-7d9",
			namespace: "payments",
			cpuMillicores: 250,
			memoryBytes: 128 * 1024 * 1024,
			sourcePods: ["api-0", "api-1"],
		},
	],
	warnings: [],
};

const pod: ResourceSummary = {
	cluster: "kind-dev",
	kind: "Pod",
	name: "api-0",
	namespace: "payments",
	age: "1m",
};

describe("resource metrics helpers", () => {
	test("formats cpu and memory samples for dense table cells", () => {
		expect(formatCpuMillicores(125)).toBe("125m");
		expect(formatCpuMillicores(1500)).toBe("1.50 cores");
		expect(formatMemoryBytes(64 * 1024 * 1024)).toBe("64 Mi");
		expect(formatCompactResourceMetrics(metrics.pods[0])).toBe("125m / 64 Mi");
	});

	test("indexes workloads and Pods by read context and resource identity", () => {
		const otherPod = { ...metrics.pods[0], cpuMillicores: 250 };
		const index = resourceMetricIndex([
			{ clusterContext: metrics.cluster, summary: metrics },
			{ clusterContext: "alias", summary: { ...metrics, pods: [otherPod], workloads: [] } },
		]);
		expect(resourceMetricFor(index, pod)).toBe(metrics.pods[0]);
		expect(resourceMetricFor(index, { ...pod, cluster: "alias" })).toBe(otherPod);
		expect(resourceMetricFor(index, { ...pod, namespace: "other" })).toBeUndefined();
		expect(resourceMetricFor(index, { ...pod, kind: "ReplicaSet", name: "api-7d9" })?.sourcePods).toEqual(["api-0", "api-1"]);
	});

	test("describes unavailable metrics states without failing the resource view", () => {
		const card = (availability: ResourceMetricsSummary["availability"]) =>
			metricsCard([{ clusterContext: "kind-dev", summary: { ...metrics, availability } }]);
		expect(card({ status: "available" })).toEqual({ message: "metrics available", contexts: [] });
		expect(card({ status: "forbidden" }).message).toBe("metrics API forbidden");
		expect(card({ status: "noSamples" }).message).toBe("no metrics samples yet");
		expect(card({ status: "unavailable", message: "metrics API unreachable" }).message).toBe(
			"metrics API unreachable",
		);
	});
});
