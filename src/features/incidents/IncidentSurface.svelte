<script lang="ts">
	import { createQuery } from "@tanstack/svelte-query";
	import type { HealthFilter } from "@/features/resources";
	import type { PathStateDetailTab } from "@/lib/path-state";
	import { createTauriClient, listIncidentCockpit } from "@/lib/tauri";
	import type { IncidentCockpitSummary, ResourceSummary } from "@/lib/types";
	import type { SavedWorkspace } from "@/lib/workspace-model";
	import type { IncidentFilter } from "./helpers";
	import IncidentView from "./IncidentView.svelte";
	import { buildIncidentQueryState, buildIncidentSurfaceState } from "./surfaceState";

	let {
		workspace,
		sourceReady,
		kubeconfigSourceKey,
		incidentFilter = $bindable("all" as IncidentFilter),
		selectedResource = null,
		onOpenResources,
		onResourceInspect,
		onResourceSelect,
	}: {
		workspace: SavedWorkspace;
		sourceReady: boolean;
		kubeconfigSourceKey?: string;
		incidentFilter?: IncidentFilter;
		selectedResource?: ResourceSummary | null;
		onOpenResources: (
			namespace?: string | string[],
			initialSearch?: string,
			initialGitOpsFilter?: string,
			initialHealthFilter?: HealthFilter,
		) => void;
		onResourceInspect: (resource: ResourceSummary, detailTab?: PathStateDetailTab) => void;
		onResourceSelect: (resource: ResourceSummary) => void;
	} = $props();

	const client = createTauriClient();
	const queryState = $derived(
		buildIncidentQueryState(workspace, sourceReady, kubeconfigSourceKey),
	);
	const incidentsQuery = createQuery<IncidentCockpitSummary>(() => ({
		queryKey: queryState.queryKey,
		queryFn: loadIncidents,
		enabled: queryState.enabled,
		staleTime: 15_000,
	}));
	const surfaceState = $derived(
		buildIncidentSurfaceState(
			incidentsQuery.data?.items ?? [],
			incidentFilter,
			selectedResource,
		),
	);
	const incidentCounts = $derived(surfaceState.counts);
	const incidentFilterOptions = $derived(surfaceState.filterOptions);
	const incidentGroups = $derived(surfaceState.groups);
	const selectedIncident = $derived(surfaceState.selectedIncident);
	const emptyState = $derived(surfaceState.emptyState);
	const visibleIncidentCount = $derived(surfaceState.visibleCount);

	async function loadIncidents(): Promise<IncidentCockpitSummary> {
		const summaries = await Promise.all(
			queryState.fetchPlans.map((plan) =>
				listIncidentCockpit(client, plan.clusterContext, plan.requests, kubeconfigSourceKey),
			),
		);
		return {
			cluster: workspace.scope.clusterContext,
			generatedAt: new Date().toISOString(),
			requestedScope: summaries.flatMap((summary) => summary.requestedScope),
			items: summaries.flatMap((summary) => summary.items),
			warnings: summaries.flatMap((summary) =>
				summary.warnings.map((warning) => `${summary.cluster}: ${warning}`),
			),
		};
	}
</script>

<IncidentView
	{workspace}
	{incidentsQuery}
	{incidentCounts}
	bind:incidentFilter
	{incidentFilterOptions}
	{incidentGroups}
	{selectedResource}
	{selectedIncident}
	{emptyState}
	{visibleIncidentCount}
	{onOpenResources}
	{onResourceInspect}
	{onResourceSelect}
/>
