import type { HealthFilter } from "@/features/resources";
import { gitOpsOwnerLabel } from "@/features/resources/helpers";
import { buildWorkspaceFetchKeys, buildWorkspaceFetchPlans } from "@/features/workspaces";
import { queryKeys } from "@/lib/queryKeys";
import type {
	IncidentCockpitItem,
	IncidentSeverity,
	ResourceSummary,
} from "@/lib/types";
import {
	type SavedWorkspace,
	workspaceScopeContexts,
} from "@/lib/workspace-model";

export type IncidentFilter = "all" | "unhealthy" | IncidentSeverity;

export interface IncidentCounts {
	total: number;
	degraded: number;
	attention: number;
	restarted: number;
	warning: number;
}

const SEVERITY_WEIGHT: Record<IncidentSeverity, number> = {
	degraded: 4,
	attention: 3,
	restarted: 2,
	warning: 1,
};

function latestSignalTime(item: IncidentCockpitItem): number {
	const value = item.latestSignalAt ?? item.latestWarningEvent?.lastSeenAt;
	if (!value) return 0;
	const time = new Date(value).getTime();
	return Number.isNaN(time) ? 0 : time;
}

export function incidentGroupLabel(resource: ResourceSummary): string {
	const gitOpsLabel = gitOpsOwnerLabel(resource);
	if (gitOpsLabel) return gitOpsLabel;
	if (resource.helmRelease) return `Helm release: ${resource.helmRelease}`;
	return "Unmanaged resources";
}

export function filterIncidentItems(
	items: IncidentCockpitItem[],
	filter: IncidentFilter,
): IncidentCockpitItem[] {
	if (filter === "all") return items;
	if (filter === "unhealthy") {
		return items.filter(
			(item) =>
				item.severity === "degraded" || item.severity === "attention",
		);
	}
	return items.filter((item) => item.severity === filter);
}

export function countIncidentItems(
	items: IncidentCockpitItem[],
): IncidentCounts {
	const counts: IncidentCounts = {
		total: 0,
		degraded: 0,
		attention: 0,
		restarted: 0,
		warning: 0,
	};
	for (const item of items) {
		counts.total += 1;
		counts[item.severity] += 1;
	}
	return counts;
}

export function sortIncidentItems(
	items: IncidentCockpitItem[],
): IncidentCockpitItem[] {
	return [...items].sort((a, b) => {
		const severityDelta =
			SEVERITY_WEIGHT[b.severity] - SEVERITY_WEIGHT[a.severity];
		if (severityDelta !== 0) return severityDelta;
		const recencyDelta = latestSignalTime(b) - latestSignalTime(a);
		if (recencyDelta !== 0) return recencyDelta;
		const namespaceDelta = (a.resource.namespace ?? "").localeCompare(
			b.resource.namespace ?? "",
		);
		if (namespaceDelta !== 0) return namespaceDelta;
		const kindDelta = a.resource.kind.localeCompare(b.resource.kind);
		if (kindDelta !== 0) return kindDelta;
		return a.resource.name.localeCompare(b.resource.name);
	});
}

export function groupIncidentItems(
	items: IncidentCockpitItem[],
): Array<{ label: string; items: IncidentCockpitItem[] }> {
	const groups = new Map<string, IncidentCockpitItem[]>();
	for (const item of sortIncidentItems(items)) {
		const label = incidentGroupLabel(item.resource);
		const group = groups.get(label);
		if (group) group.push(item);
		else groups.set(label, [item]);
	}
	return Array.from(groups, ([label, groupItems]) => ({
		label,
		items: groupItems,
	}));
}

export function buildIncidentQueryState(
	workspace: SavedWorkspace,
	sourceReady: boolean,
	kubeconfigSourceKey?: string,
) {
	const fetchKeys = buildWorkspaceFetchKeys(workspace.scope);
	return {
		queryKey: queryKeys.incidentCockpit(
			workspaceScopeContexts(workspace.scope).join("|"),
			fetchKeys,
			kubeconfigSourceKey,
		),
		fetchPlans: buildWorkspaceFetchPlans(workspace.scope),
		enabled: sourceReady && fetchKeys.length > 0,
	};
}

export function buildIncidentSurfaceState(
	items: IncidentCockpitItem[],
	filter: IncidentFilter,
	selectedKey?: string | null,
) {
	const counts = countIncidentItems(items);
	const visibleItems = sortIncidentItems(filterIncidentItems(items, filter));
	const groups = groupIncidentItems(visibleItems);
	const resolvedSelectedKey = reconcileIncidentSelection(visibleItems, selectedKey);
	return {
		counts,
		filterOptions: buildIncidentFilterOptions(counts),
		groups,
		visibleItems,
		visibleCount: visibleItems.length,
		selectedKey: resolvedSelectedKey,
		selectedIncident: visibleItems.find((item) => incidentItemKey(item) === resolvedSelectedKey) ?? null,
		emptyState:
			counts.total === 0 ? "clean" : groups.length === 0 ? "filtered" : "ready",
	} as const;
}

export function reconcileIncidentSelection(
	items: IncidentCockpitItem[],
	selectedKey: string | null | undefined,
): string | null {
	if (selectedKey && items.some((item) => incidentItemKey(item) === selectedKey)) {
		return selectedKey;
	}
	const first = sortIncidentItems(items)[0];
	return first ? incidentItemKey(first) : null;
}

export interface IncidentFilterOption {
	id: IncidentFilter;
	label: string;
	count: number;
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
