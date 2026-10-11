import assert from "node:assert/strict";
import { bench, describe } from "vitest";
import {
	describeMetricsAvailability,
	mergeResourceMetrics,
	mergeTopologyMetrics,
	resourceMetricIndex,
} from "@/lib/resource-metrics";
import type {
	ResourceMetricSummary,
	ResourceMetricsSummary,
	ResourceSummary,
	ResourceTopology,
	TopologyNode,
} from "@/lib/types";

const total = 10_000;
const kinds = ["Pod", "Node", "Deployment"] as const;

function metric(index: number): ResourceMetricSummary {
	const kind = kinds[index % kinds.length] ?? "Pod";
	return {
		kind,
		cluster: "prod",
		name: `${kind.toLowerCase()}-${index}`,
		namespace: kind === "Node" ? null : `namespace-${index % 100}`,
		cpuMillicores: index % 1_000,
		memoryBytes: (index % 512) * 1024 * 1024,
		sourcePods: [],
	};
}

const allMetrics = Array.from({ length: total }, (_, index) => metric(index));
const metrics: ResourceMetricsSummary = {
	cluster: "prod",
	availability: { status: "available" },
	pods: allMetrics.filter((value) => value.kind === "Pod"),
	nodes: allMetrics.filter((value) => value.kind === "Node"),
	workloads: allMetrics.filter((value) => value.kind === "Deployment"),
	warnings: [],
};
const sparseMetrics: ResourceMetricsSummary = {
	...metrics,
	pods: metrics.pods.filter((_, index) => index % 100 === 0),
	nodes: metrics.nodes.filter((_, index) => index % 100 === 0),
	workloads: metrics.workloads.filter((_, index) => index % 100 === 0),
	warnings: ["Some resource metric samples were unavailable."],
};
const unavailableMetrics: ResourceMetricsSummary = {
	cluster: "prod",
	availability: { status: "unavailable", message: "metrics API unreachable" },
	pods: [],
	nodes: [],
	workloads: [],
	warnings: ["The metrics API could not be reached."],
};
const unsupportedMetrics: ResourceMetricsSummary = {
	cluster: "prod",
	availability: { status: "forbidden" },
	pods: [],
	nodes: [],
	workloads: [],
	warnings: ["The metrics API denied this request."],
};
const resources: ResourceSummary[] = allMetrics.map((value) => ({
	cluster: value.cluster,
	apiVersion: value.kind === "Pod" || value.kind === "Node" ? "v1" : "apps/v1",
	kind: value.kind,
	name: value.name,
	namespace: value.namespace,
	age: "1m",
	health: "healthy",
}));
const topology: ResourceTopology = {
	nodes: resources.map(
		(summary, index): TopologyNode => ({
			id: `node-${index}`,
			kind: summary.kind,
			name: summary.name,
			namespace: summary.namespace,
			health: "healthy",
			selectable: true,
			summary,
		}),
	),
	edges: [],
	warnings: [],
};
const sparseIndex = resourceMetricIndex(sparseMetrics);
const unavailableIndex = resourceMetricIndex(unavailableMetrics);
const unsupportedIndex = resourceMetricIndex(unsupportedMetrics);
const unmatchedResources = resources.map((resource) => ({
	...resource,
	name: `unmatched-${resource.name}`,
}));
const emptyIndex = resourceMetricIndex(undefined);
const sparseTableResult = mergeResourceMetrics(
	resources,
	sparseMetrics,
	sparseIndex,
);
const sparseTopologyResult = mergeTopologyMetrics(
	topology,
	sparseMetrics,
	sparseIndex,
);
const unavailableTableResult = mergeResourceMetrics(
	resources,
	unavailableMetrics,
	unavailableIndex,
);
const unsupportedTopologyResult = mergeTopologyMetrics(
	topology,
	unsupportedMetrics,
	unsupportedIndex,
);
const sparseMetricsSnapshot = JSON.stringify(sparseMetrics);
const unavailableMetricsSnapshot = JSON.stringify(unavailableMetrics);
const unsupportedMetricsSnapshot = JSON.stringify(unsupportedMetrics);

assert.equal(
	metrics.pods.length + metrics.nodes.length + metrics.workloads.length,
	total,
);
assert(sparseIndex.size > 0 && sparseIndex.size < total);
assert(sparseTableResult.some((row) => row.metrics !== undefined));
assert(sparseTableResult.some((row) => row.metrics === undefined));
assert(sparseTableResult.some((row, index) => row === resources[index] && !row.metrics));
assert.equal(sparseTopologyResult?.edges, topology.edges);
assert.equal(topology.nodes.some((node) => node.metrics !== undefined), false);
assert.equal(topology.edges.length, 0);
assert.equal(unavailableIndex.size, 0);
assert.equal(unsupportedIndex.size, 0);
assert.equal(unavailableTableResult, resources);
assert.equal(unsupportedTopologyResult, topology);
assert.equal(
	describeMetricsAvailability(unavailableMetrics.availability),
	"metrics API unreachable",
);
assert.equal(
	describeMetricsAvailability(unsupportedMetrics.availability),
	"metrics API forbidden",
);
assert.deepEqual(unavailableMetrics.warnings, ["The metrics API could not be reached."]);
assert.deepEqual(unsupportedMetrics.warnings, ["The metrics API denied this request."]);
assert.equal(JSON.stringify(sparseMetrics), sparseMetricsSnapshot);
assert.equal(JSON.stringify(unavailableMetrics), unavailableMetricsSnapshot);
assert.equal(JSON.stringify(unsupportedMetrics), unsupportedMetricsSnapshot);
assert.equal(
	mergeResourceMetrics(unmatchedResources, metrics, emptyIndex),
	unmatchedResources,
);

describe("F05 resource metric arrival and retained sparse joins (10k rows)", () => {
	bench("F05 cold shared index + table and topology joins", () => {
		const index = resourceMetricIndex(metrics);
		mergeResourceMetrics(resources, metrics, index);
		mergeTopologyMetrics(topology, metrics, index);
	});

	bench("F05 retained sparse index + table and topology updates", () => {
		mergeResourceMetrics(resources, sparseMetrics, sparseIndex);
		mergeTopologyMetrics(topology, sparseMetrics, sparseIndex);
		mergeResourceMetrics(resources, unavailableMetrics, unavailableIndex);
		mergeTopologyMetrics(topology, unsupportedMetrics, unsupportedIndex);
	});

	bench(
		"F05 empty index preserves unmatched table and topology references",
		() => {
			mergeResourceMetrics(unmatchedResources, metrics, emptyIndex);
			mergeTopologyMetrics(topology, metrics, emptyIndex);
		},
	);
});
