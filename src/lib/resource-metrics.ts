import type {
	ResourceMetricSummary,
	ResourceMetricsAvailability,
	ResourceMetricsSummary,
	ResourceSummary,
} from "./types";

export type ResourceMetricIndex = ReadonlyMap<string, ResourceMetricSummary>;

// The cluster keeps same-named objects of two workspace contexts apart.
function metricKey(cluster: string, kind: string, namespace: string | null | undefined, name: string) {
	return `${cluster}:${kind}:${namespace ?? ""}:${name}`;
}

/** One context's metrics read. Its samples join rows of the context that was read. */
export interface ContextMetrics {
	clusterContext: string;
	summary: ResourceMetricsSummary | undefined;
}

export function resourceMetricIndex(reads: readonly ContextMetrics[]): ResourceMetricIndex {
	const index = new Map<string, ResourceMetricSummary>();
	for (const { clusterContext, summary } of reads) {
		if (!summary) continue;
		for (const metrics of [summary.pods, summary.nodes, summary.workloads]) {
			for (const metric of metrics) {
				index.set(metricKey(clusterContext, metric.kind, metric.namespace, metric.name), metric);
			}
		}
	}
	return index;
}

export const TOPOLOGY_METRICS = Symbol("topology metrics");

export function resourceMetricFor(
	index: ResourceMetricIndex | undefined,
	resource: Pick<ResourceSummary, "cluster" | "kind" | "namespace" | "name">,
	fallback?: ResourceMetricSummary,
): ResourceMetricSummary | undefined {
	return index?.get(metricKey(resource.cluster, resource.kind, resource.namespace, resource.name)) ?? fallback;
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

/** Why a context's metrics are missing, or null when they are available. */
function metricsIssue(availability: ResourceMetricsAvailability): string | null {
	if (availability.status === "available") return null;
	if (availability.status === "forbidden") return "metrics API forbidden";
	if (availability.status === "noSamples") return "no metrics samples yet";
	return availability.message ?? "metrics API unavailable";
}

/** One context's settled metrics read. */
export type ContextMetricsRead =
	| { clusterContext: string; summary: ResourceMetricsSummary }
	| { clusterContext: string; failure: "forbidden" | "unavailable" };

export interface MetricsCard {
	message: string;
	/** "<context>: <issue>" for each context without metrics, listed only when several contexts are read. */
	contexts: string[];
}

/**
 * The Resource metrics card for settled reads in rank order: "metrics available"
 * while any context has metrics, else the first context's issue.
 */
export function metricsCard(reads: readonly ContextMetricsRead[]): MetricsCard {
	const issues = reads.map((read) => ({
		clusterContext: read.clusterContext,
		issue: "summary" in read
			? metricsIssue(read.summary.availability)
			: `metrics API ${read.failure}`,
	}));
	const first = issues[0]?.issue;
	return {
		message: first && issues.every(({ issue }) => issue) ? first : "metrics available",
		contexts: issues.length > 1
			? issues.flatMap(({ clusterContext, issue }) => (issue ? [`${clusterContext}: ${issue}`] : []))
			: [],
	};
}
