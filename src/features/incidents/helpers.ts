import type {
	IncidentCockpitItem,
	IncidentSeverity,
	ResourceSummary,
} from "@/lib/types";

export type IncidentFilter = "all" | IncidentSeverity;

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

export function incidentGroupLabel(resource: ResourceSummary): string {
	if (resource.argoApp) return `Argo app: ${resource.argoApp}`;
	if (resource.helmRelease) return `Helm release: ${resource.helmRelease}`;
	if (resource.ownerRef) return `Owner: ${resource.ownerRef}`;
	return "Unowned resources";
}

export function filterIncidentItems(
	items: IncidentCockpitItem[],
	filter: IncidentFilter,
): IncidentCockpitItem[] {
	if (filter === "all") return items;
	return items.filter((item) => item.severity === filter);
}

export function countIncidentItems(items: IncidentCockpitItem[]): IncidentCounts {
	return items.reduce<IncidentCounts>(
		(counts, item) => ({
			...counts,
			total: counts.total + 1,
			[item.severity]: counts[item.severity] + 1,
		}),
		{ total: 0, degraded: 0, attention: 0, restarted: 0, warning: 0 },
	);
}

export function sortIncidentItems(
	items: IncidentCockpitItem[],
): IncidentCockpitItem[] {
	return [...items].sort((a, b) => {
		const severityDelta =
			SEVERITY_WEIGHT[b.severity] - SEVERITY_WEIGHT[a.severity];
		if (severityDelta !== 0) return severityDelta;
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
