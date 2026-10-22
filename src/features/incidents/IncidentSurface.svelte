<script lang="ts">
	import { createQuery, skipToken } from "@tanstack/svelte-query";
	import {
		buildResourceDetailReadSpec,
		resourceDetailsQuery,
	} from "@/features/resource-detail";
	import { resourceTopologyQuery, type HealthFilter } from "@/features/resources";
	import type { PathStateDetailTab } from "@/lib/path-state";
	import { queryKeys } from "@/lib/queryKeys";
	import { getSettingsSnapshot } from "@/lib/settings-store";
	import { createTauriClient, listIncidentCockpit } from "@/lib/tauri";
	import type {
		IncidentCockpitSummary,
		ResourceDetailsFull,
		ResourceSummary,
		ResourceTopology,
	} from "@/lib/types";
	import type { SavedWorkspace } from "@/lib/workspace-model";
	import IncidentView from "./IncidentView.svelte";
	import {
		buildIncidentGuidance,
		resolveIncidentOwner,
		type IncidentEnrichmentState,
		type IncidentOwnerResolution,
	} from "./guidance";
	import {
		buildIncidentQueryState,
		buildIncidentSurfaceState,
		type IncidentFilter,
	} from "./model";

	let {
		workspace,
		sourceReady,
		kubeconfigSourceKey,
		incidentFilter = $bindable<IncidentFilter>("all"),
		onOpenResources,
		onResourceInspect,
		onResourceSelect,
	}: {
		workspace: SavedWorkspace;
		sourceReady: boolean;
		kubeconfigSourceKey?: string;
		incidentFilter?: IncidentFilter;
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
	const settings = getSettingsSnapshot();
	const yamlViewMode = settings.yamlViewModeDefault;
	const yamlEncoding = settings.yamlEncodingDefault;
	let selectedIncidentKey = $state<string | null>(null);
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
			selectedIncidentKey,
		),
	);
	const incidentCounts = $derived(surfaceState.counts);
	const incidentFilterOptions = $derived(surfaceState.filterOptions);
	const incidentGroups = $derived(surfaceState.groups);
	const selectedKey = $derived(surfaceState.selectedKey);
	const selectedIncident = $derived(surfaceState.selectedIncident);
	const selectedResource = $derived(selectedIncident?.resource ?? null);
	const emptyState = $derived(surfaceState.emptyState);
	const visibleIncidentCount = $derived(surfaceState.visibleCount);

	$effect(() => {
		if (selectedIncidentKey !== selectedKey) selectedIncidentKey = selectedKey;
	});

	const detailReadSpec = $derived(
		selectedResource
			? buildResourceDetailReadSpec(
					selectedResource,
					kubeconfigSourceKey,
					yamlViewMode,
					yamlEncoding,
				)
			: null,
	);
	const detailsEnabled = $derived(Boolean(sourceReady && detailReadSpec?.detailsEnabled));
	const topologyEnabled = $derived(
		Boolean(
			sourceReady &&
				selectedResource?.kind === "Pod" &&
				selectedResource.namespace,
		),
	);
	// The same details and topology reads as Resource Detail and the Browser map share their cache.
	const detailsQuery = createQuery<ResourceDetailsFull>(() =>
		detailReadSpec
			? resourceDetailsQuery(client, detailReadSpec, detailsEnabled)
			: { queryKey: queryKeys.idle("incident-details"), queryFn: skipToken },
	);
	const topologyQuery = createQuery<ResourceTopology>(() =>
		topologyEnabled && selectedResource?.namespace
			? resourceTopologyQuery(client, {
					clusterContext: selectedResource.cluster,
					namespaces: [selectedResource.namespace],
					mode: "ownership",
					kubeconfigSourceKey,
				})
			: { queryKey: queryKeys.idle("incident-topology"), queryFn: skipToken },
	);
	const detailsState = $derived<IncidentEnrichmentState>(
		!detailsEnabled
			? "idle"
			: detailsQuery.isError
				? "error"
				: detailsQuery.isPending || detailsQuery.isFetching
					? "loading"
					: "ready",
	);
	const topologyState = $derived<IncidentEnrichmentState>(
		!topologyEnabled
			? "idle"
			: topologyQuery.isError
				? "error"
				: topologyQuery.isPending || topologyQuery.isFetching
					? "loading"
					: "ready",
	);
	const ownerResolution = $derived.by<IncidentOwnerResolution>(() => {
		if (!selectedResource) {
			return { directOwner: null, workloadOwner: null, chain: [] };
		}
		return resolveIncidentOwner(selectedResource, topologyQuery.data);
	});
	const guidance = $derived(
		selectedIncident
			? buildIncidentGuidance(
					selectedIncident,
					detailsQuery.data,
					ownerResolution,
					detailsState,
					topologyState,
				)
			: null,
	);

	function selectIncident(key: string) {
		selectedIncidentKey = key;
	}

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
	{selectedKey}
	{selectedIncident}
	{guidance}
	{detailsState}
	{topologyState}
	{emptyState}
	{visibleIncidentCount}
	onIncidentSelect={selectIncident}
	{onOpenResources}
	{onResourceInspect}
	{onResourceSelect}
/>
