import { expect, test } from "bun:test";
import {
	mergeResourceMetrics,
	mergeTopologyMetrics,
	resourceMetricIndex,
} from "./resource-metrics";
import { buildResourceSearchIndex } from "@/features/resources/helpers";
import type {
	ResourceMetricSummary,
	ResourceSummary,
	ResourceTopology,
} from "./types";

// Failure modes: unchanged samples churn identities, changed/stale samples are hidden,
// equal values from different sources are confused, and stale search text survives a row replacement.
test("metrics retain equal rows but publish every changed sample field and remove missing samples", () => {
	const row: ResourceSummary = {
		cluster: "dev",
		namespace: "ns",
		kind: "Pod",
		name: "pod",
		age: "1m",
		health: "healthy",
	};
	const metric: ResourceMetricSummary = {
		cluster: "dev",
		namespace: "ns",
		kind: "Pod",
		name: "pod",
		cpuMillicores: 1,
		sourcePods: ["pod"],
	};
	const input = [row];
	const index = (value: ResourceMetricSummary) =>
		resourceMetricIndex({
			cluster: "dev",
			availability: { status: "available" },
			pods: [value],
			nodes: [],
			workloads: [],
			warnings: [],
		});
	const first = mergeResourceMetrics(input, undefined, index(metric));
	expect(
		mergeResourceMetrics(
			input,
			undefined,
			index({ ...metric, sourcePods: ["pod"] }),
		),
	).toBe(first);
	for (const changed of [
		{ ...metric, cpuMillicores: 2 },
		{ ...metric, sampledAt: "now" },
		{ ...metric, sourcePods: ["another"] },
	]) {
		const result = mergeResourceMetrics(input, undefined, index(changed));
		expect(result[0]).not.toBe(first[0]);
		expect(result[0].metrics).toEqual(changed);
	}
	expect(mergeResourceMetrics(input, undefined, new Map())).toBe(input);
	expect(row.metrics).toBeUndefined();
});

test("search entries retain identity for unchanged rows and replace changed search terms", () => {
	const row: ResourceSummary = {
		cluster: "dev",
		namespace: null,
		kind: "Pod",
		name: "old-name",
		age: "1m",
		health: "healthy",
	};
	const first = buildResourceSearchIndex([row])[0];
	expect(buildResourceSearchIndex([row])[0]).toBe(first);
	const changed = buildResourceSearchIndex([{ ...row, name: "new-name" }])[0];
	expect(changed.searchText).toContain("new-name");
	expect(changed.searchText).not.toContain("old-name");
});

test("unchanged topology samples preserve the graph, changed samples update node and summary", () => {
	const row: ResourceSummary = {
		cluster: "dev",
		namespace: null,
		kind: "Node",
		name: "node",
		age: "1m",
		health: "healthy",
	};
	const topology: ResourceTopology = {
		nodes: [{ id: "node", ...row, selectable: true, summary: row }],
		edges: [],
		warnings: [],
	};
	const metric: ResourceMetricSummary = {
		cluster: "dev",
		namespace: null,
		kind: "Node",
		name: "node",
		memoryBytes: 1,
		sourcePods: [],
	};
	const first = mergeTopologyMetrics(
		topology,
		undefined,
		new Map([["Node::node", metric]]),
	);
	expect(
		mergeTopologyMetrics(
			topology,
			undefined,
			new Map([["Node::node", { ...metric }]]),
		),
	).toBe(first);
	const changed = mergeTopologyMetrics(
		topology,
		undefined,
		new Map([["Node::node", { ...metric, memoryBytes: 2 }]]),
	);
	expect(changed?.nodes[0].summary.metrics?.memoryBytes).toBe(2);
	expect(changed?.edges).toBe(topology.edges);
});
