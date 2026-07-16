<script lang="ts">
	import { createQuery, useQueryClient } from "@tanstack/svelte-query";
	import {
		dynamicResourceKindFromSummary,
		shouldFetchResourceDetails,
	} from "@/features/resource-detail";
	import type { HealthFilter } from "@/features/resources";
	import { createCancellableRequest, createCancelScope } from "@/lib/cancellable-loads";
	import { diagnosticLog } from "@/lib/diagnostics";
	import type { PathStateDetailTab } from "@/lib/path-state";
	import { queryKeys } from "@/lib/queryKeys";
	import { getSettingsSnapshot } from "@/lib/settings-store";
	import {
		cancelBackendRequests,
		createTauriClient,
		getDynamicResourceDetails,
		getResourceDetails,
		isAppError,
		listIncidentCockpit,
		listResourceTopology,
	} from "@/lib/tauri";
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
		incidentFilter = $bindable("all" as IncidentFilter),
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
	const queryClient = useQueryClient();
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

	const dynamicKind = $derived(
		selectedResource ? dynamicResourceKindFromSummary(selectedResource) : null,
	);
	const dynamicKindKey = $derived(
		dynamicKind
			? `${dynamicKind.group}/${dynamicKind.version}/${dynamicKind.kind}/${dynamicKind.plural}/${dynamicKind.namespaced}`
			: "",
	);
	const detailsEnabled = $derived(
		Boolean(
			sourceReady &&
				selectedResource &&
				shouldFetchResourceDetails(selectedResource),
		),
	);
	const topologyEnabled = $derived(
		Boolean(
			sourceReady &&
				selectedResource?.kind === "Pod" &&
				selectedResource.namespace,
		),
	);
	const detailsQueryKey = $derived(
		selectedResource
			? queryKeys.resourceDetails(
					selectedResource,
					dynamicKindKey,
					kubeconfigSourceKey,
					yamlViewMode,
					yamlEncoding,
				)
			: (["resource-details", "incident-idle"] as const),
	);
	const topologyQueryKey = $derived(
		topologyEnabled && selectedResource?.namespace
			? queryKeys.resourceTopology(
					selectedResource.cluster,
					[selectedResource.namespace],
					"ownership",
					kubeconfigSourceKey,
				)
			: (["resource-topology", "incident-idle"] as const),
	);
	const detailsCancelScope = $derived(createCancelScope("resource-details", detailsQueryKey));
	const topologyCancelScope = $derived(
		createCancelScope("resource-topology", topologyQueryKey),
	);
	const pendingCancelTimers = new Map<string, ReturnType<typeof setTimeout>>();

	function cancelPendingBackendScope(cancelScope: string) {
		const timer = pendingCancelTimers.get(cancelScope);
		if (!timer) return;
		clearTimeout(timer);
		pendingCancelTimers.delete(cancelScope);
	}

	function scheduleBackendScopeCancel(
		cancelScope: string,
		queryKey: readonly unknown[],
		event: string,
	) {
		cancelPendingBackendScope(cancelScope);
		const timer = setTimeout(() => {
			pendingCancelTimers.delete(cancelScope);
			const query = queryClient.getQueryCache().find({ queryKey, exact: true });
			if ((query?.getObserversCount() ?? 0) > 0) return;
			void queryClient.cancelQueries({ queryKey, exact: true });
			void cancelBackendRequests(client, cancelScope).catch((error: unknown) => {
				diagnosticLog(`${event}.error`, {
					error: error instanceof Error ? error.message : String(error),
				});
			});
		}, 0);
		pendingCancelTimers.set(cancelScope, timer);
	}

	$effect(() => {
		const currentDetailsCancelScope = detailsCancelScope;
		const currentDetailsQueryKey = detailsQueryKey;
		const currentTopologyCancelScope = topologyCancelScope;
		const currentTopologyQueryKey = topologyQueryKey;
		cancelPendingBackendScope(currentDetailsCancelScope);
		cancelPendingBackendScope(currentTopologyCancelScope);
		return () => {
			scheduleBackendScopeCancel(
				currentDetailsCancelScope,
				currentDetailsQueryKey,
				"incidents.details.cancel",
			);
			scheduleBackendScopeCancel(
				currentTopologyCancelScope,
				currentTopologyQueryKey,
				"incidents.topology.cancel",
			);
		};
	});

	const detailsQuery = createQuery<ResourceDetailsFull>(() => ({
		queryKey: detailsQueryKey,
		queryFn: async () => {
			const resource = selectedResource;
			if (!resource) throw new Error("Incident detail requested without a selected resource.");
			try {
				return dynamicKind
					? await getDynamicResourceDetails(
							client,
							resource.cluster,
							dynamicKind,
							resource.name,
							resource.namespace ?? undefined,
							kubeconfigSourceKey,
							yamlViewMode,
							yamlEncoding,
							createCancellableRequest(detailsCancelScope, "details"),
						)
					: await getResourceDetails(
							client,
							resource.cluster,
							resource.kind,
							resource.name,
							resource.namespace ?? undefined,
							kubeconfigSourceKey,
							yamlViewMode,
							yamlEncoding,
							createCancellableRequest(detailsCancelScope, "details"),
						);
			} catch (error) {
				if (isAppError(error) && error.kind === "cancelled") {
					diagnosticLog("incidents.details.cancel", { key: selectedKey });
				}
				throw error;
			}
		},
		enabled: detailsEnabled,
		retry: false,
		staleTime: 30_000,
	}));
	const topologyQuery = createQuery<ResourceTopology>(() => ({
		queryKey: topologyQueryKey,
		queryFn: async () => {
			const resource = selectedResource;
			if (!resource?.namespace) {
				throw new Error("Incident topology requested without a namespaced resource.");
			}
			try {
				return await listResourceTopology(
					client,
					resource.cluster,
					[resource.namespace],
					"ownership",
					kubeconfigSourceKey,
					createCancellableRequest(topologyCancelScope, "topology"),
				);
			} catch (error) {
				if (isAppError(error) && error.kind === "cancelled") {
					diagnosticLog("incidents.topology.cancel", { key: selectedKey });
				}
				throw error;
			}
		},
		enabled: topologyEnabled,
		retry: false,
		staleTime: 30_000,
	}));
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
