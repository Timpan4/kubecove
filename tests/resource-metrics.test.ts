import { describe, expect, test } from "bun:test";
import {
	describeMetricsAvailability,
	formatCompactResourceMetrics,
	formatCpuMillicores,
	formatMemoryBytes,
	mergeResourceMetrics,
	mergeTopologyMetrics,
	resourceMetricIndex,
} from "../src/lib/resource-metrics";
import type {
	ResourceMetricsSummary,
	ResourceSummary,
	ResourceTopology,
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

	test("merges pod and workload metrics onto matching resources", () => {
		const rows = mergeResourceMetrics(
			[
				pod,
				{ ...pod, kind: "ReplicaSet", name: "api-7d9" },
				{ ...pod, name: "worker-0" },
			],
			metrics,
		);

		expect(rows[0].metrics?.cpuMillicores).toBe(125);
		expect(rows[1].metrics?.sourcePods).toEqual(["api-0", "api-1"]);
		expect(rows[2].metrics).toBeUndefined();
	});

	test("merges metrics onto topology nodes and node summaries", () => {
		const topology: ResourceTopology = {
			nodes: [
				{
					id: "pod",
					kind: "Pod",
					name: "api-0",
					namespace: "payments",
					health: "healthy",
					selectable: true,
					summary: pod,
				},
			],
			edges: [],
			warnings: [],
		};

		const merged = mergeTopologyMetrics(topology, metrics);

		expect(merged?.nodes[0].metrics?.memoryBytes).toBe(64 * 1024 * 1024);
		expect(merged?.nodes[0].summary.metrics?.cpuMillicores).toBe(125);
	});

	test("shared metric index matches default table and topology merges", () => {
		const topology: ResourceTopology = {
			nodes: [
				{
					id: "pod",
					kind: "Pod",
					name: "api-0",
					namespace: "payments",
					health: "healthy",
					selectable: true,
					summary: pod,
				},
			],
			edges: [],
			warnings: [],
		};
		const rows = [pod, { ...pod, kind: "ReplicaSet", name: "api-7d9" }];
		const index = resourceMetricIndex(metrics);

		expect(mergeResourceMetrics(rows, metrics, index)).toEqual(
			mergeResourceMetrics(rows, metrics),
		);
		expect(mergeTopologyMetrics(topology, metrics, index)).toEqual(
			mergeTopologyMetrics(topology, metrics),
		);
	});

	test("keeps inputs when metrics index is empty", () => {
		const rows = [pod];
		const topology: ResourceTopology = { nodes: [], edges: [], warnings: [] };
		const emptyIndex = resourceMetricIndex(undefined);

		expect(mergeResourceMetrics(rows, undefined, emptyIndex)).toBe(rows);
		expect(mergeTopologyMetrics(topology, undefined, emptyIndex)).toBe(topology);
	});

	test("describes unavailable metrics states without failing the resource view", () => {
		expect(describeMetricsAvailability({ status: "available" })).toBeNull();
		expect(describeMetricsAvailability({ status: "forbidden" })).toBe(
			"metrics API forbidden",
		);
		expect(describeMetricsAvailability({ status: "noSamples" })).toBe(
			"no metrics samples yet",
		);
		expect(
			describeMetricsAvailability({
				status: "unavailable",
				message: "metrics API unavailable",
			}),
		).toBe("metrics API unavailable");
	});
});
