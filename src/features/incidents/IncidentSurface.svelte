<script lang="ts">
	import { createQuery } from "@tanstack/svelte-query";
	import type { HealthFilter } from "@/features/resources/helpers";
	import { buildWorkspaceFetchKeys, buildWorkspaceFetchPlans } from "@/features/workspaces";
	import type { PathStateDetailTab } from "@/lib/path-state";
	import { queryKeys } from "@/lib/queryKeys";
	import { createTauriClient, listIncidentCockpit } from "@/lib/tauri";
	import type { IncidentCockpitSummary, ResourceSummary } from "@/lib/types";
	import { workspaceScopeContexts, type SavedWorkspace } from "@/lib/workspace-model";
	import {
		countIncidentItems,
		filterIncidentItems,
		groupIncidentItems,
		type IncidentFilter,
	} from "./helpers";
	import IncidentView from "./IncidentView.svelte";
	import { buildIncidentFilterOptions } from "./model";

	let {
		workspace,
		sourceReady,
		kubeconfigSourceKey,
		initialIncidentFilter = "all",
		selectedResource = null,
		onIncidentFilterChange = () => {},
		onOpenResources,
		onResourceInspect,
		onResourceSelect,
	}: {
		workspace: SavedWorkspace;
		sourceReady: boolean;
		kubeconfigSourceKey?: string;
		initialIncidentFilter?: IncidentFilter;
		selectedResource?: ResourceSummary | null;
		onIncidentFilterChange?: (filter: IncidentFilter) => void;
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
	function initialIncidentFilterValue(): IncidentFilter {
		return initialIncidentFilter;
	}
	let incidentFilter = $state<IncidentFilter>(initialIncidentFilterValue());
	const fetchKeys = $derived(buildWorkspaceFetchKeys(workspace.scope));
	const workspaceContextKey = $derived(workspaceScopeContexts(workspace.scope).join("|"));
	const incidentsQuery = createQuery<IncidentCockpitSummary>(() => ({
		queryKey: queryKeys.incidentCockpit(workspaceContextKey, fetchKeys, kubeconfigSourceKey),
		queryFn: loadIncidents,
		enabled: sourceReady && fetchKeys.length > 0,
		staleTime: 15_000,
	}));
	const visibleIncidents = $derived(
		filterIncidentItems(incidentsQuery.data?.items ?? [], incidentFilter),
	);
	const incidentCounts = $derived(countIncidentItems(incidentsQuery.data?.items ?? []));
	const incidentFilterOptions = $derived(buildIncidentFilterOptions(incidentCounts));
	const incidentGroups = $derived(groupIncidentItems(visibleIncidents));

	$effect(() => onIncidentFilterChange(incidentFilter));

	async function loadIncidents(): Promise<IncidentCockpitSummary> {
		const summaries = await Promise.all(
			buildWorkspaceFetchPlans(workspace.scope).map((plan) =>
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
	{onOpenResources}
	{onResourceInspect}
	{onResourceSelect}
/>
