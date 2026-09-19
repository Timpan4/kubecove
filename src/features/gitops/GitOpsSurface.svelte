<script lang="ts">
	import ResourceRefreshButton from "@/components/ResourceRefreshButton.svelte";
	import { observeResourceScope } from "@/lib/resource-watch";
	import { refreshCurrentView } from "@/lib/resource-refresh";
	import { createQueries, createQuery, useQueryClient } from "@tanstack/svelte-query";
	import type { HealthFilter } from "@/features/resources";
	import type { PathStateDetailTab } from "@/lib/path-state";
	import { queryKeys } from "@/lib/queryKeys";
	import {
		createTauriClient,
		detectArgoCD,
		detectFlux,
		listNamespaces,
		listArgoApplicationSets,
		listArgoApplications,
		listArgoAppProjects,
		listFluxResources,
	} from "@/lib/tauri";
	import type {
		ArgoApplicationSetSummary,
		ArgoApplicationSummary,
		ArgoAppProjectSummary,
		ResourceSummary,
	} from "@/lib/types";
	import type { TreeNodeId } from "@/lib/tree-nav";
	import type { SavedWorkspace } from "@/lib/workspace-model";
	import { createArgoListFreshness } from "./argo-application-freshness";
	import { argoTrackingKey, getArgoOperationTracker } from "./argo-operation-tracker";
	import GitOpsView from "./GitOpsView.svelte";
	import {
		buildGitOpsRailItems,
		buildGitOpsSelections,
		buildGitOpsTable,
		gitOpsActiveRailKey,
		gitOpsSelectionKey,
		gitOpsUnavailableProvider,
		type GitOpsSelection,
	} from "./surfaceModel";
	import { buildGitOpsReadState } from "./surfaceState";
	import {
		argoApplicationResourceNavigation,
		resolveTargetGitOpsSelection,
	} from "./surfaceSelection";

	let {
		workspace,
		sourceReady,
		kubeconfigSourceKey,
		selectedNode,
		targetGitOpsApplication = null,
		selectedGitOpsItem = $bindable(null),
		onTargetGitOpsApplicationResolved = () => {},
		onOpenResources,
		onResourceInspect,
	}: {
		workspace: SavedWorkspace;
		sourceReady: boolean;
		kubeconfigSourceKey?: string;
		selectedNode: TreeNodeId | null;
		targetGitOpsApplication?: string | null;
		selectedGitOpsItem?: GitOpsSelection | null;
		onTargetGitOpsApplicationResolved?: () => void;
		onOpenResources: (
			namespace?: string | string[],
			initialSearch?: string,
			initialGitOpsFilter?: string,
			initialHealthFilter?: HealthFilter,
			gitOpsFocusApplication?: ArgoApplicationSummary | null,
		) => void;
		onResourceInspect: (resource: ResourceSummary, detailTab?: PathStateDetailTab) => void;
	} = $props();

	const client = createTauriClient();
	const queryClient = useQueryClient();
	const tracker = getArgoOperationTracker(queryClient);
	function trackedOperationLabel(selection: GitOpsSelection): string | null {
		if (selection.type !== "argoApp") return null;
		const operation = $tracker.get(argoTrackingKey({ name: selection.item.name, namespace: selection.item.namespace, context, workspaceId: workspace.id }, kubeconfigSourceKey));
		if (!operation) return null;
		const action = operation.request.action === "hardRefresh" ? "Hard refresh" : operation.request.action === "sync" ? "Sync" : "Refresh";
		return `${action}: ${operation.phase}`;
	}
	const context = $derived(workspace.scope.clusterContext);
	let realtimeMessage = $state("Starting live updates");
	let realtimeError = $state<string | null>(null);

	const namespacesQuery = createQuery(() => ({
		queryKey: queryKeys.namespaces(context, kubeconfigSourceKey),
		queryFn: () => listNamespaces(client, context, kubeconfigSourceKey),
		enabled: sourceReady,
		retry: false,
	}));
	const argoDetectionQuery = createQuery<boolean>(() => ({
		queryKey: queryKeys.argoDetect(context, kubeconfigSourceKey),
		queryFn: () => detectArgoCD(client, context, kubeconfigSourceKey),
		enabled: sourceReady && !namespacesQuery.isPending,
		staleTime: 60_000,
	}));
	const fluxDetectionQuery = createQuery(() => ({
		queryKey: queryKeys.fluxDetect(context, kubeconfigSourceKey),
		queryFn: () => detectFlux(client, context, kubeconfigSourceKey),
		enabled: sourceReady && !namespacesQuery.isPending,
		staleTime: 60_000,
	}));
	const watchKeys = $derived([
		...(argoDetectionQuery.data === true ? [
			{ resourceKind: { kind: "Application", apiVersion: "argoproj.io/v1alpha1", plural: "applications", namespaced: true } },
			{ resourceKind: { kind: "ApplicationSet", apiVersion: "argoproj.io/v1alpha1", plural: "applicationsets", namespaced: true } },
			{ resourceKind: { kind: "AppProject", apiVersion: "argoproj.io/v1alpha1", plural: "appprojects", namespaced: true } },
		] : []),
		...(fluxDetectionQuery.data?.kinds ?? []).map((kind) => ({ resourceKind: kind })),
	]);
	function refreshView() {
		return refreshCurrentView({ client, queryClient, clusterContext: context, kubeconfigEnvVar: kubeconfigSourceKey, keys: watchKeys, namespaces: [] });
	}
	const argoAppsQuery = createQuery<ArgoApplicationSummary[]>(() => ({
		queryKey: queryKeys.argoApps(context, kubeconfigSourceKey),
		queryFn: () => listArgoApplications(client, context, kubeconfigSourceKey),
		enabled: sourceReady && argoDetectionQuery.data === true,
		staleTime: 15_000,
	}));
	const argoAppSetsQuery = createQuery<ArgoApplicationSetSummary[]>(() => ({
		queryKey: queryKeys.argoAppSets(context, kubeconfigSourceKey),
		queryFn: () => listArgoApplicationSets(client, context, kubeconfigSourceKey),
		enabled: sourceReady && argoDetectionQuery.data === true,
		staleTime: 15_000,
	}));
	const argoProjectsQuery = createQuery<ArgoAppProjectSummary[]>(() => ({
		queryKey: queryKeys.argoAppProjects(context, kubeconfigSourceKey),
		queryFn: () => listArgoAppProjects(client, context, kubeconfigSourceKey),
		enabled: sourceReady && argoDetectionQuery.data === true,
		staleTime: 15_000,
	}));
	const fluxResourceQueries = createQueries(() => ({
		queries: (fluxDetectionQuery.data?.kinds ?? []).map((kind) => ({
			queryKey: queryKeys.fluxResources(context, kind, kubeconfigSourceKey),
			queryFn: () => listFluxResources(client, context, kind, kubeconfigSourceKey),
			enabled: sourceReady && fluxDetectionQuery.data?.detected === true,
			staleTime: 15_000,
		})),
	}));
	const readState = $derived(buildGitOpsReadState({
		argoDetection: argoDetectionQuery,
		fluxDetection: fluxDetectionQuery,
		argoApps: argoAppsQuery,
		argoAppSets: argoAppSetsQuery,
		argoProjects: argoProjectsQuery,
		fluxResources: fluxResourceQueries,
	}));
	const gitOpsQuery = $derived(readState.query);
	const gitOpsProviderError = $derived(readState.providerError);
	const gitOpsListError = $derived(readState.listError);
	const gitOpsData = $derived(readState.query.data);
	const gitOpsTable = $derived(gitOpsData ? buildGitOpsTable(gitOpsData, selectedNode) : null);
	const unavailableProvider = $derived(
		gitOpsData ? gitOpsUnavailableProvider(gitOpsData, selectedNode) : null,
	);
	const selections = $derived(gitOpsData ? buildGitOpsSelections(gitOpsData, selectedNode) : []);
	const railItems = $derived(gitOpsData ? buildGitOpsRailItems(gitOpsData) : []);
	const activeRailKey = $derived(gitOpsData ? gitOpsActiveRailKey(gitOpsData, selectedNode) : "");
	const selectedGitOpsItemKey = $derived(
		selectedGitOpsItem ? gitOpsSelectionKey(selectedGitOpsItem) : "",
	);

	$effect(() => {
		const target = resolveTargetGitOpsSelection(
			selections,
			targetGitOpsApplication,
			Boolean(gitOpsData),
		);
		if (target.selection) {
			selectedGitOpsItem = target.selection;
			onTargetGitOpsApplicationResolved();
			return;
		}
		if (target.shouldResolve) onTargetGitOpsApplicationResolved();
		if (
			selectedGitOpsItem &&
			!selections.some((item) => gitOpsSelectionKey(item) === selectedGitOpsItemKey)
		) {
			selectedGitOpsItem = null;
		}
	});

	$effect(() => {
		if (!sourceReady || watchKeys.length === 0) return;
		const clusterContext = context;
		const source = kubeconfigSourceKey;
		const keys = watchKeys;
		const freshness = createArgoListFreshness((queryKey) => void queryClient.invalidateQueries({ queryKey }), source);
		const stop = observeResourceScope({ client, clusterContext, keys, kubeconfigEnvVar: source,
			onState: (state) => { realtimeMessage = state.message; realtimeError = state.error; },
			reload: () => refreshCurrentView({ client, queryClient, clusterContext, kubeconfigEnvVar: source, keys, namespaces: [] }),
			onChange: (event) => {
				freshness.handle(event);
				if (event.type === "resourceChanged") {
					for (const kind of keys) {
						if (kind.resourceKind.kind === event.target.kind && kind.resourceKind.apiVersion !== "argoproj.io/v1alpha1") {
							void queryClient.invalidateQueries({ queryKey: ["flux-resources", queryKeys.argoApps(clusterContext, source)[1], clusterContext] });
						}
					}
				}
			},
		});
		return () => { stop(); freshness.dispose(); };
	});

	function openSelectedArgoApplicationResources(selectionOverride?: GitOpsSelection) {
		const selection = selectionOverride ?? selectedGitOpsItem;
		if (!selection) return;
		const navigation = argoApplicationResourceNavigation(selection);
		if (!navigation) return;
		onOpenResources(
			navigation.namespaces,
			"",
			navigation.gitOpsFilter,
			navigation.healthFilter,
			navigation.focusApplication,
		);
	}
	function gitOpsStatusClass(status: string | null | undefined) {
		if (status === "Synced" || status === "Healthy") {
			return "border-emerald-500/40 bg-emerald-500/10 text-emerald-700 dark:text-emerald-300";
		}
		if (status === "Degraded" || status === "Missing") {
			return "border-destructive/40 bg-destructive/10 text-destructive";
		}
		if (status === "OutOfSync" || status === "Progressing" || status === "Unknown") {
			return "border-amber-500/40 bg-amber-500/10 text-amber-700 dark:text-amber-300";
		}
		return "";
	}
</script>

<div class="flex flex-wrap items-center justify-between gap-2 pb-2">
	<span class="text-xs text-muted-foreground" role="status">{realtimeMessage}{realtimeError ? ': ' + realtimeError : ''}</span>
	{#key context + kubeconfigSourceKey}<ResourceRefreshButton onRefresh={refreshView} disabled={!sourceReady} />{/key}
</div>
<GitOpsView
	{gitOpsQuery}
	{gitOpsProviderError}
	{gitOpsListError}
	gitOpsUnavailableProvider={unavailableProvider}
	{gitOpsTable}
	gitOpsSelections={selections}
	gitOpsRailItems={railItems}
	gitOpsActiveRailKey={activeRailKey}
	bind:selectedGitOpsItem
	{selectedGitOpsItemKey}
	{openSelectedArgoApplicationResources}
	{onResourceInspect}
	{gitOpsStatusClass}
	{trackedOperationLabel}
/>
