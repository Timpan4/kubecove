<script lang="ts">
	import { createQuery } from "@tanstack/svelte-query";
	import {
		GitOpsSurface,
		selectedGitOpsApplicationName,
		type GitOpsSelection,
	} from "@/features/gitops";
	import { HelmSurface, selectedHelmReleasePath } from "@/features/helm";
	import { IncidentSurface, type IncidentFilter } from "@/features/incidents";
	import { LiveSessionsSurface } from "@/features/live-sessions";
	import { RbacSurface } from "@/features/rbac";
	import type { HealthFilter } from "@/features/resources";
	import {
		createTauriClient,
		getKubeconfigSources,
	} from "@/lib/tauri";
	import type {
		ArgoApplicationSummary,
		HelmReleaseSummary,
		ResourceSummary,
	} from "@/lib/types";
	import type { TreeNodeId } from "@/lib/tree-nav";
	import type { PathStateDetailTab, PathStateSurfacesState } from "@/lib/path-state";
	import type { SavedWorkspace } from "@/lib/workspace-model";
	import SettingsSurface from "./SettingsSurface.svelte";
	import { treeNodeForResource, type WorkspaceViewMode } from "./workspaceNavigation";

	let {
		workspace,
		viewMode,
		selectedNode,
		targetHelmRelease,
		targetGitOpsApplication,
		selectedResource,
		initialIncidentFilter = "all",
		initialPathState = null,
		onOpenResources,
		onResourceInspect,
		onResourceSelect,
		onTargetHelmReleaseResolved,
		onTargetGitOpsApplicationResolved,
		onPathStateChange = () => {},
		onCloseSettings = () => {},
	}: {
		workspace: SavedWorkspace;
		viewMode: WorkspaceViewMode;
		selectedNode: TreeNodeId | null;
		targetHelmRelease?: { name: string; namespace?: string | null } | null;
		targetGitOpsApplication?: string | null;
		selectedResource?: ResourceSummary | null;
		initialIncidentFilter?: IncidentFilter;
		initialPathState?: PathStateSurfacesState | null;
		onOpenResources: (
			namespace?: string | string[],
			initialSearch?: string,
			initialGitOpsFilter?: string,
			initialHealthFilter?: HealthFilter,
			gitOpsFocusApplication?: ArgoApplicationSummary | null,
		) => void;
		onResourceInspect: (resource: ResourceSummary, detailTab?: PathStateDetailTab) => void;
		onResourceSelect: (resource: ResourceSummary, nodeId: TreeNodeId) => void;
		onTargetHelmReleaseResolved?: () => void;
		onTargetGitOpsApplicationResolved?: () => void;
		onPathStateChange?: (state: PathStateSurfacesState) => void;
		onCloseSettings?: () => void;
	} = $props();

	const client = createTauriClient();
	function initialIncidentFilterValue(): IncidentFilter {
		return initialPathState?.incidentFilter ?? initialIncidentFilter;
	}

	function initialHelmSearchValue(): string {
		return initialPathState?.helmSearch ?? "";
	}

	let incidentFilter = $state<IncidentFilter>(initialIncidentFilterValue());
	let selectedGitOpsItem = $state<GitOpsSelection | null>(null);
	let helmSearch = $state(initialHelmSearchValue());
	let selectedHelmRelease = $state<HelmReleaseSummary | null>(null);

	const sourceQuery = createQuery(() => ({
		queryKey: ["kubeconfig-sources"],
		queryFn: () => getKubeconfigSources(client),
		staleTime: 30_000,
	}));
	const sourceReady = $derived(sourceQuery.isSuccess || sourceQuery.isError);
	const kubeconfigSourceKey = $derived(sourceQuery.data?.sourceKey);
	const showKubeconfigSourceLabels = $derived(sourceQuery.data?.showSourceLabels ?? false);

	$effect(() => {
		if (viewMode === "incidents") incidentFilter = initialIncidentFilter;
	});
	$effect(() => {
		if (viewMode !== "argo") selectedGitOpsItem = null;
	});
	$effect(() => {
		if (viewMode !== "helm") selectedHelmRelease = null;
	});

	$effect(() => {
		onPathStateChange({
			incidentFilter,
			helmSearch,
			selectedHelmRelease: selectedHelmReleasePath(selectedHelmRelease),
			selectedGitOpsApplication: selectedGitOpsApplicationName(selectedGitOpsItem),
		});
	});


</script>

{#if viewMode === "argo"}
	{#key workspace.id}
		<GitOpsSurface
			{workspace}
			{sourceReady}
			{kubeconfigSourceKey}
			{selectedNode}
			{targetGitOpsApplication}
			bind:selectedGitOpsItem
			{onTargetGitOpsApplicationResolved}
			{onOpenResources}
			{onResourceInspect}
		/>
	{/key}
{:else if viewMode === "helm"}
	{#key workspace.id}
		<HelmSurface
			{workspace}
			{sourceReady}
			{kubeconfigSourceKey}
			{targetHelmRelease}
			bind:helmSearch
			bind:selectedHelmRelease
			{onTargetHelmReleaseResolved}
			{onOpenResources}
		/>
	{/key}
{:else if viewMode === "rbac"}
	{#key workspace.id}
		<RbacSurface {workspace} {sourceReady} {kubeconfigSourceKey} {selectedNode} />
	{/key}
{:else if viewMode === "incidents"}
	{#key workspace.id}
		<IncidentSurface
			{workspace}
			{sourceReady}
			{kubeconfigSourceKey}
			bind:incidentFilter
			{selectedResource}
			{onOpenResources}
			{onResourceInspect}
			onResourceSelect={(resource) => onResourceSelect(resource, treeNodeForResource(resource))}
		/>
	{/key}
{:else if viewMode === "portForwards"}
	{#key workspace.id}
		<LiveSessionsSurface
			{workspace}
			{sourceReady}
			{kubeconfigSourceKey}
			{showKubeconfigSourceLabels}
		/>
	{/key}
{:else if viewMode === "settings"}
	<SettingsSurface onBack={onCloseSettings} />
{/if}
