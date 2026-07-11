import { buildWorkspaceFetchKeys, buildWorkspaceFetchPlans } from "@/features/workspaces";
import { queryKeys } from "@/lib/queryKeys";
import type { IncidentCockpitItem, ResourceSummary } from "@/lib/types";
import { workspaceScopeContexts, type SavedWorkspace } from "@/lib/workspace-model";
import {
	countIncidentItems,
	filterIncidentItems,
	groupIncidentItems,
	type IncidentFilter,
} from "./helpers";
import { buildIncidentFilterOptions, isIncidentResourceSelected } from "./model";

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
	selectedResource?: ResourceSummary | null,
) {
	const counts = countIncidentItems(items);
	const visibleItems = filterIncidentItems(items, filter);
	const groups = groupIncidentItems(visibleItems);
	return {
		counts,
		filterOptions: buildIncidentFilterOptions(counts),
		groups,
		visibleCount: visibleItems.length,
		selectedIncident:
			visibleItems.find((item) => isIncidentResourceSelected(item, selectedResource)) ?? null,
		emptyState: counts.total === 0 ? "clean" : groups.length === 0 ? "filtered" : "ready",
	} as const;
}
