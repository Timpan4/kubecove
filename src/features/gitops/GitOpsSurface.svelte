<script lang="ts">
	import { createQueries, createQuery } from "@tanstack/svelte-query";
	import { argoApplicationResourceNamespaces, type HealthFilter } from "@/features/resources";
	import type { PathStateDetailTab } from "@/lib/path-state";
	import { queryKeys } from "@/lib/queryKeys";
	import {
		createTauriClient,
		detectArgoCD,
		detectFlux,
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
	const context = $derived(workspace.scope.clusterContext);
	const argoDetectionQuery = createQuery<boolean>(() => ({
		queryKey: queryKeys.argoDetect(context, kubeconfigSourceKey),
		queryFn: () => detectArgoCD(client, context, kubeconfigSourceKey),
		enabled: sourceReady,
		staleTime: 60_000,
	}));
	const fluxDetectionQuery = createQuery(() => ({
		queryKey: queryKeys.fluxDetect(context, kubeconfigSourceKey),
		queryFn: () => detectFlux(client, context, kubeconfigSourceKey),
		enabled: sourceReady,
		staleTime: 60_000,
	}));
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
		if (targetGitOpsApplication) {
			const match = selections.find(
				(item) => item.type === "argoApp" && item.item.name === targetGitOpsApplication,
			);
			if (match) {
				selectedGitOpsItem = match;
				onTargetGitOpsApplicationResolved();
				return;
			}
			if (gitOpsData) onTargetGitOpsApplicationResolved();
		}
		if (
			selectedGitOpsItem &&
			!selections.some((item) => gitOpsSelectionKey(item) === selectedGitOpsItemKey)
		) {
			selectedGitOpsItem = null;
		}
	});

	function openSelectedArgoApplicationResources(selectionOverride?: GitOpsSelection) {
		const selection = selectionOverride ?? selectedGitOpsItem;
		if (!selection || selection.type !== "argoApp") return;
		onOpenResources(
			argoApplicationResourceNamespaces(selection.item),
			"",
			"",
			"all",
			selection.item,
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
/>
