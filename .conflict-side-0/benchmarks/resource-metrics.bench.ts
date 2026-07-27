import { bench, describe } from "vitest";
import {
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
const resources: ResourceSummary[] = allMetrics.map((value) => ({
	cluster: value.cluster,
	apiVersion: value.kind === "Pod" || value.kind === "Node" ? "v1" : "apps/v1",
	kind: value.kind,
	name: value.name,
	namespace: value.namespace,
	age: "1m",
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
describe("resource metric merges (10k metrics)", () => {
	bench("table and topology merges (duplicate indexes)", () => {
		mergeResourceMetrics(resources, metrics);
		mergeTopologyMetrics(topology, metrics);
	});

	bench("table and topology merges (one shared index)", () => {
		const index = resourceMetricIndex(metrics);
		mergeResourceMetrics(resources, metrics, index);
		mergeTopologyMetrics(topology, metrics, index);
	});
});
