import type {
	ResourceMetricSummary,
	ResourceMetricsAvailability,
	ResourceMetricsSummary,
	ResourceSummary,
	ResourceTopology,
	TopologyNode,
} from "./types";

export type ResourceMetricIndex = ReadonlyMap<string, ResourceMetricSummary>;

const metricRows = new WeakMap<ResourceSummary, ResourceSummary>();
const metricTables = new WeakMap<ResourceSummary[], { index: ResourceMetricIndex; rows: ResourceSummary[] }>();
const metricNodes = new WeakMap<TopologyNode, TopologyNode>();
const metricTopologies = new WeakMap<ResourceTopology, { index: ResourceMetricIndex; topology: ResourceTopology }>();

function sameMetric(left: ResourceMetricSummary | undefined, right: ResourceMetricSummary): boolean {
	return left === right || Boolean(left && left.cluster === right.cluster && left.kind === right.kind &&
		left.namespace === right.namespace && left.name === right.name &&
		left.cpuMillicores === right.cpuMillicores && left.memoryBytes === right.memoryBytes &&
		left.sampledAt === right.sampledAt && left.sourcePods.length === right.sourcePods.length &&
		left.sourcePods.every((pod, index) => pod === right.sourcePods[index]));
}

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
	const cachedTable = metricTables.get(resources);
	if (cachedTable?.index === index) return cachedTable.rows;
	const previous = cachedTable?.rows ?? resources;
	const rows: ResourceSummary[] = [];
	let unchanged = true;
	for (const resource of resources) {
		const metric = index.get(
			metricKey(resource.kind, resource.namespace, resource.name),
		);
		let row = resource;
		if (metric && !sameMetric(resource.metrics, metric)) {
			const cached = metricRows.get(resource);
			row = cached && sameMetric(cached.metrics, metric) ? cached : { ...resource, metrics: metric };
			metricRows.set(resource, row);
		}
		if (row !== previous[rows.length]) unchanged = false;
		rows.push(row);
	}
	const result = unchanged ? previous : rows;
	metricTables.set(resources, { index, rows: result });
	return result;
}

function mergeTopologyNodeMetrics(
	node: TopologyNode,
	index: ResourceMetricIndex,
): TopologyNode {
	const metric = index.get(metricKey(node.kind, node.namespace, node.name));
	if (!metric) return node;
	if (sameMetric(node.metrics, metric) && sameMetric(node.summary.metrics, metric)) return node;
	const cached = metricNodes.get(node);
	if (cached && sameMetric(cached.metrics, metric)) return cached;
	const merged = {
		...node,
		metrics: metric,
		summary: { ...node.summary, metrics: metric },
	};
	metricNodes.set(node, merged);
	return merged;
}

export function mergeTopologyMetrics(
	topology: ResourceTopology | undefined,
	metrics: ResourceMetricsSummary | undefined,
	index: ResourceMetricIndex = resourceMetricIndex(metrics),
): ResourceTopology | undefined {
	if (!topology || index.size === 0) return topology;
	const cachedTopology = metricTopologies.get(topology);
	if (cachedTopology?.index === index) return cachedTopology.topology;
	const previous = cachedTopology?.topology ?? topology;
	const nodes: TopologyNode[] = [];
	let unchanged = true;
	for (const node of topology.nodes) {
		const merged = mergeTopologyNodeMetrics(node, index);
		if (merged !== previous.nodes[nodes.length]) unchanged = false;
		nodes.push(merged);
	}
	const merged = {
		...topology,
		nodes,
	};
	const result = unchanged ? previous : merged;
	metricTopologies.set(topology, { index, topology: result });
	return result;
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
