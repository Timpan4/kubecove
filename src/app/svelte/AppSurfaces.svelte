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
	import { RbacSurface, type RbacCockpitState, type RbacView } from "@/features/rbac";
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
import type { RbacVerifierHandoff } from "@/features/rbac";
	import SettingsSurface from "./SettingsSurface.svelte";
	import { treeNodeForResource, type WorkspaceViewMode } from "./workspaceNavigation";

	let {
		workspace,
		viewMode,
		selectedNode,
		targetHelmRelease,
		targetGitOpsApplication,
		initialIncidentFilter = "all",
		initialPathState = null,
		onOpenResources,
		onResourceInspect,
		onResourceSelect,
		onTargetHelmReleaseResolved,
		onTargetGitOpsApplicationResolved,
		onPathStateChange = () => {},
		rbacVerifierHandoff,
		onRbacVerifierHandoffConsumed,
		onRbacViewChange,
		onRbacVerifierReturn,
		rbacVerifierReturnLabel,
		onCloseSettings = () => {},
	}: {
		workspace: SavedWorkspace;
		viewMode: WorkspaceViewMode;
		selectedNode: TreeNodeId | null;
		targetHelmRelease?: { name: string; namespace?: string | null } | null;
		targetGitOpsApplication?: string | null;
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
		rbacVerifierHandoff?: RbacVerifierHandoff;
		onRbacVerifierHandoffConsumed?: () => void;
		onRbacViewChange?: (view: RbacView) => void;
		onRbacVerifierReturn?: () => void;
		rbacVerifierReturnLabel?: string;
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
	function initialRbacStateValue(): RbacCockpitState | undefined {
		return initialPathState?.rbac
			? {
				riskBucket: initialPathState.rbac.riskBucket,
				selectedObjectKey: initialPathState.rbac.selectedObjectKey ?? undefined,
			}
			: undefined;
	}
	let rbacState = $state<RbacCockpitState | undefined>(initialRbacStateValue());

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
			rbac: rbacState
				? {
					riskBucket: rbacState.riskBucket ?? "all",
					selectedObjectKey: rbacState.selectedObjectKey ?? null,
				}
				: null,
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
	{#key `${workspace.id}:${workspace.scope.clusterContext}:${kubeconfigSourceKey ?? ""}`}
		<RbacSurface
			{workspace}
			{sourceReady}
			{kubeconfigSourceKey}
			{selectedNode}
			initialState={rbacState}
			onStateChange={(state) => (rbacState = state)}
			onViewChange={onRbacViewChange}
			verifierHandoff={rbacVerifierHandoff}
			onVerifierHandoffConsumed={onRbacVerifierHandoffConsumed}
			onVerifierReturn={onRbacVerifierReturn}
			verifierReturnLabel={rbacVerifierReturnLabel}
		/>
	{/key}
{:else if viewMode === "incidents"}
	{#key workspace.id}
		<IncidentSurface
			{workspace}
			{sourceReady}
			{kubeconfigSourceKey}
			bind:incidentFilter
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
