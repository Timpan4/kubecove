import type {
	IncidentCockpitItem,
	IncidentSeverity,
	ResourceSummary,
} from "@/lib/types";
import { gitOpsOwnerLabel } from "@/features/resources/helpers";

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
		return items.filter((item) => item.severity === "degraded" || item.severity === "attention");
	}
	return items.filter((item) => item.severity === filter);
}

export function countIncidentItems(items: IncidentCockpitItem[]): IncidentCounts {
	const counts: IncidentCounts = { total: 0, degraded: 0, attention: 0, restarted: 0, warning: 0 };
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
		groups.set(label, [...(groups.get(label) ?? []), item]);
	}
	return Array.from(groups, ([label, groupItems]) => ({
		label,
		items: groupItems,
	}));
}
