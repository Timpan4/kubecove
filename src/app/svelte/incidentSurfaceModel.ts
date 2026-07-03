import type { IncidentFilter } from "@/features/incidents/helpers";
import type { HealthFilter } from "@/features/resources/helpers";
import type { PathStateDetailTab } from "@/lib/path-state";
import type { IncidentCockpitItem, ResourceSummary } from "@/lib/types";

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
	const extra = item.warningEventCount > 1 ? `, ${item.warningEventCount} warnings` : "";
	return `${event.reason} (${event.lastSeen}${extra})`;
}

export function incidentResourceKey(resource: ResourceSummary): string {
	return [
		resource.cluster,
		resource.apiVersion ?? "",
		resource.kind,
		resource.namespace ?? "",
		resource.name,
	].join(":");
}

export function incidentItemKey(item: IncidentCockpitItem): string {
	return incidentResourceKey(item.resource);
}

export function isIncidentResourceSelected(
	item: IncidentCockpitItem,
	selectedResource: ResourceSummary | null | undefined,
): boolean {
	return selectedResource ? incidentResourceKey(item.resource) === incidentResourceKey(selectedResource) : false;
}

export function incidentCaseTitle(item: IncidentCockpitItem): string {
	const resource = `${item.resource.kind}/${item.resource.name}`;
	const signal = item.signals[0]?.label ?? item.latestWarningEvent?.reason ?? incidentSeverityLabel(item);
	return `${resource}: ${signal}`;
}

export function incidentCaseSummary(item: IncidentCockpitItem): string {
	const signal = item.signals[0];
	if (signal?.message) return signal.message;
	if (item.latestWarningEvent?.message) return item.latestWarningEvent.message;
	return `${incidentSeverityLabel(item)} signal in ${incidentScopeLabel(item)}.`;
}

export function incidentKnownSummary(item: IncidentCockpitItem): string {
	const status = [
		item.resource.status,
		item.resource.ready ? `Ready ${item.resource.ready}` : undefined,
		item.resource.restarts && item.resource.restarts > 0 ? `${item.resource.restarts} restarts` : undefined,
	]
		.filter(Boolean)
		.join(", ");
	return status || incidentSignalSummary(item);
}

export function incidentMissingSummary(item: IncidentCockpitItem): string {
	if (item.latestWarningEvent) return "Check Events for repeat count and source details.";
	if (item.resource.kind === "Pod") return "Open Logs if status and events do not explain the signal.";
	return "Open related resources if owner context is not enough.";
}

export function incidentNextSummary(item: IncidentCockpitItem): string {
	if (item.resource.kind === "Pod") return "Inspect details, then Events or Logs.";
	if (item.latestWarningEvent) return "Inspect details, then Events.";
	return "Inspect details, then open Resources for neighbors.";
}

export interface IncidentDetailPivot {
	id: "details" | "events" | "logs" | "yaml";
	label: string;
	tab: PathStateDetailTab;
	enabled: boolean;
}

export function incidentDetailPivots(item: IncidentCockpitItem): IncidentDetailPivot[] {
	const hasEvents = item.warningEventCount > 0 || item.signals.some((signal) => signal.kind === "event");
	const canShowLogs = item.resource.kind === "Pod" && Boolean(item.resource.namespace);
	return [
		{ id: "details", label: "Inspect", tab: "details", enabled: true },
		{ id: "events", label: "Events", tab: "events", enabled: hasEvents },
		{ id: "logs", label: "Logs", tab: "logs", enabled: canShowLogs },
		{ id: "yaml", label: "YAML", tab: "yaml", enabled: true },
	];
}
