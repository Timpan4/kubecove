import type { IncidentFilter } from "@/features/incidents/helpers";
import type { HealthFilter } from "@/features/resources/helpers";
import type { IncidentCockpitItem } from "@/lib/types";

export interface IncidentFilterOption {
	id: IncidentFilter;
	label: string;
	count: number;
}

interface IncidentCounts {
	total: number;
	degraded: number;
	attention: number;
	restarted: number;
	warning: number;
}

export function buildIncidentFilterOptions(
	counts: IncidentCounts,
): IncidentFilterOption[] {
	return [
		{ id: "all", label: "All", count: counts.total },
		{ id: "unhealthy", label: "Unhealthy", count: counts.degraded + counts.attention },
		{ id: "degraded", label: "Degraded", count: counts.degraded },
		{ id: "attention", label: "Needs attention", count: counts.attention },
		{ id: "restarted", label: "Restarted", count: counts.restarted },
		{ id: "warning", label: "Warnings", count: counts.warning },
	];
}

export function incidentResourcesHealthFilter(
	filter: IncidentFilter,
): HealthFilter {
	return filter === "degraded" ||
		filter === "attention" ||
		filter === "restarted" ||
		filter === "unhealthy"
		? filter
		: "all";
}

export function incidentSeverityLabel(item: IncidentCockpitItem): string {
	if (item.severity === "degraded") return "Degraded";
	if (item.severity === "attention") return "Needs attention";
	if (item.severity === "restarted") return "Restarted";
	return "Warning";
}

export function incidentScopeLabel(item: IncidentCockpitItem): string {
	const namespace = item.resource.namespace ?? "cluster";
	return `${namespace} / ${item.resource.cluster}`;
}

export function incidentSignalSummary(item: IncidentCockpitItem): string {
	const visible = item.signals.slice(0, 3);
	if (visible.length === 0) {
		return item.latestWarningEvent?.message ?? "-";
	}
	const summary = visible
		.map((signal) =>
			signal.message ? `${signal.label}: ${signal.message}` : signal.label,
		)
		.join(" | ");
	const hiddenCount = item.signals.length - visible.length;
	return hiddenCount > 0 ? `${summary} | +${hiddenCount} more` : summary;
}

export function incidentWarningSummary(item: IncidentCockpitItem): string {
	const event = item.latestWarningEvent;
	if (!event) return "-";
	return `${event.reason} (${event.lastSeen})`;
}
