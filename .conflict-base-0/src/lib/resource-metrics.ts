import type {
	ResourceMetricSummary,
	ResourceMetricsAvailability,
	ResourceMetricsSummary,
	ResourceSummary,
	ResourceTopology,
	TopologyNode,
} from "./types";

export type ResourceMetricIndex = ReadonlyMap<string, ResourceMetricSummary>;

function metricKey(kind: string, namespace: string | null | undefined, name: string) {
	return `${kind}:${namespace ?? ""}:${name}`;
}

export function resourceMetricIndex(
	summary: ResourceMetricsSummary | undefined,
): ResourceMetricIndex {
	const index = new Map<string, ResourceMetricSummary>();
	if (!summary) return index;
	for (const metrics of [summary.pods, summary.nodes, summary.workloads]) {
		for (const metric of metrics) {
			index.set(metricKey(metric.kind, metric.namespace, metric.name), metric);
		}
	}
	return index;
}

export function mergeResourceMetrics(
	resources: ResourceSummary[],
	metrics: ResourceMetricsSummary | undefined,
	index: ResourceMetricIndex = resourceMetricIndex(metrics),
): ResourceSummary[] {
	if (index.size === 0) return resources;
	return resources.map((resource) => {
		const metric = index.get(
			metricKey(resource.kind, resource.namespace, resource.name),
		);
		return metric ? { ...resource, metrics: metric } : resource;
	});
}

function mergeTopologyNodeMetrics(
	node: TopologyNode,
	index: ResourceMetricIndex,
): TopologyNode {
	const metric = index.get(metricKey(node.kind, node.namespace, node.name));
	if (!metric) return node;
	return {
		...node,
		metrics: metric,
		summary: { ...node.summary, metrics: metric },
	};
}

export function mergeTopologyMetrics(
	topology: ResourceTopology | undefined,
	metrics: ResourceMetricsSummary | undefined,
	index: ResourceMetricIndex = resourceMetricIndex(metrics),
): ResourceTopology | undefined {
	if (!topology || index.size === 0) return topology;
	return {
		...topology,
		nodes: topology.nodes.map((node) => mergeTopologyNodeMetrics(node, index)),
	};
}

export function formatCpuMillicores(value: number | null | undefined): string {
	if (value === undefined || value === null) return "—";
	if (value >= 1000) return `${(value / 1000).toFixed(2)} cores`;
	if (value >= 10) return `${Math.round(value)}m`;
	return `${value.toFixed(1)}m`;
}

export function formatMemoryBytes(value: number | null | undefined): string {
	if (value === undefined || value === null) return "—";
	const units = ["B", "Ki", "Mi", "Gi", "Ti"];
	let current = value;
	let index = 0;
	while (current >= 1024 && index < units.length - 1) {
		current /= 1024;
		index += 1;
	}
	const precision = index === 0 || current >= 10 ? 0 : 1;
	return `${current.toFixed(precision)} ${units[index]}`;
}

export function formatCompactResourceMetrics(
	metric: ResourceMetricSummary | null | undefined,
): string | null {
	if (!metric) return null;
	const parts: string[] = [];
	if (metric.cpuMillicores !== undefined) {
		parts.push(formatCpuMillicores(metric.cpuMillicores));
	}
	if (metric.memoryBytes !== undefined) {
		parts.push(formatMemoryBytes(metric.memoryBytes));
	}
	return parts.length > 0 ? parts.join(" / ") : null;
}

export function describeMetricsAvailability(
	availability: ResourceMetricsAvailability | undefined,
): string | null {
	if (!availability || availability.status === "available") return null;
	if (availability.status === "forbidden") return "metrics API forbidden";
	if (availability.status === "noSamples") return "no metrics samples yet";
	return availability.message ?? "metrics API unavailable";
}
