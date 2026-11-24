import assert from "node:assert/strict";
import { describe } from "vitest";
import {
	type ContextMetrics,
	resourceMetricFor,
	metricsCard,
	resourceMetricIndex,
} from "@/lib/resource-metrics";
import { PAGE_SIZE } from "@/features/resources/constants";
import { buildResourceTableDisplayFromMembership, buildResourceTableMembership, buildResourceTablePage, buildResourceTableProjection, sortResourceTableRows } from "@/features/resources/resourceTableModel";
import { buildFlowTopologyLayoutState, selectFlowTopology } from "@/features/resources/topology-view";
import type {
	ResourceMetricSummary,
	ResourceMetricsSummary,
	ResourceSummary,
	ResourceTopology,
	TopologyNode,
} from "@/lib/types";
import { benchmark } from "./benchmark";

const total = 10_000;

const oneContext = (summary: ResourceMetricsSummary): ContextMetrics[] => [
	{ clusterContext: summary.cluster, summary },
];
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
const sparseIndex = resourceMetricIndex(oneContext(sparseMetrics));
const unavailableIndex = resourceMetricIndex(oneContext(unavailableMetrics));
const unsupportedIndex = resourceMetricIndex(oneContext(unsupportedMetrics));
const projection = buildResourceTableProjection(resources);
const membership = buildResourceTableMembership(projection, { search: "", gitOpsFilter: "", healthFilter: "all" });
const byName = sortResourceTableRows(membership.filteredRows, { id: "name", desc: false });
const display = buildResourceTableDisplayFromMembership(membership, byName, {});
const page = buildResourceTablePage(display, 0);
const layout = buildFlowTopologyLayoutState(topology, "networkFlow", new Set(), null);
const selected = selectFlowTopology(layout, { selectedNodeId: topology.nodes[0].id, showFullTopologyOnSelection: true });
const sampledNodeIndex = selected.nodes.findIndex(({ data }) => data.resource && resourceMetricFor(sparseIndex, data.resource));
assert(sampledNodeIndex >= 0);
const visibleNodes = selected.nodes.slice(sampledNodeIndex, sampledNodeIndex + PAGE_SIZE);

function readVisibleSamples(index: ReturnType<typeof resourceMetricIndex>) {
	return [
		page.pageRows.map((row) => resourceMetricFor(index, row, row.metrics)),
		visibleNodes.map(({ data }) => data.resource && resourceMetricFor(index, data.resource)),
	] as const;
}

assert.equal(membership.filteredRows.length, total);
assert.equal(layout.layout.nodes.length, topology.nodes.length);
const [sparseRows, sparseNodes] = readVisibleSamples(sparseIndex);
assert(sparseRows.some((sample) => sample !== undefined));
assert(sparseRows.some((sample) => sample === undefined));
assert(sparseNodes.some((sample) => sample !== undefined));
assert(readVisibleSamples(unavailableIndex)[0].every((sample) => sample === undefined));
assert(readVisibleSamples(unsupportedIndex)[1].every((sample) => sample === undefined));
assert.equal(metricsCard([{ clusterContext: "prod", summary: unavailableMetrics }]).message, "metrics API unreachable");
assert.equal(metricsCard([{ clusterContext: "prod", summary: unsupportedMetrics }]).message, "metrics API forbidden");

const fullIndex = resourceMetricIndex(oneContext(metrics));
const byCpu = sortResourceTableRows(membership.filteredRows, { id: "cpu", desc: true }, [], fullIndex);
assert.notEqual(byCpu[0], byName[0]);
assert.equal(resourceMetricFor(fullIndex, byCpu[0])?.cpuMillicores, 999);
const byMemory = sortResourceTableRows(membership.filteredRows, { id: "memory", desc: true }, [], fullIndex);
assert.equal(resourceMetricFor(fullIndex, byMemory[0])?.memoryBytes, 511 * 1024 * 1024);
assert.strictEqual(display.filteredRows, membership.filteredRows);
assert.strictEqual(selected.nodes[0].data.resource?.metrics, undefined);
assert(resources.every((row) => row.metrics === undefined));

describe("F05 metric samples with retained table and map projections", () => {
	benchmark("F05 cold sample index and visible labels (10k samples)", () => {
		readVisibleSamples(resourceMetricIndex(oneContext(metrics)));
	});

	benchmark("F05 fixed-batch metric lookup microbenchmark for table page and map labels (not viewport coverage)", () => {
		readVisibleSamples(fullIndex);
		readVisibleSamples(sparseIndex);
		readVisibleSamples(unavailableIndex);
		readVisibleSamples(unsupportedIndex);
	});

	benchmark("F05 CPU sorting with retained table membership", () => {
		const sortedRows = sortResourceTableRows(membership.filteredRows, { id: "cpu", desc: true }, [], fullIndex);
		buildResourceTableDisplayFromMembership(membership, sortedRows, {});
	});
});
