<script module lang="ts">
	const loadGitOps = () => import("@/features/gitops/GitOpsSurface.svelte");
	const loadHelm = () => import("@/features/helm/HelmSurface.svelte");
	const loadRbac = () => import("@/features/rbac/RbacSurface.svelte");
	const loadIncidents = () => import("@/features/incidents/IncidentSurface.svelte");
	const loadLiveSessions = () => import("@/features/live-sessions/LiveSessionsSurface.svelte");
	const loadSettings = () => import("./SettingsSurface.svelte");
</script>

<script lang="ts">
	import {
		selectedGitOpsApplicationName,
		type GitOpsSelection,
	} from "@/features/gitops/surfaceSelection";
	import { selectedHelmReleasePath } from "@/features/helm/surfaceState";
	import type { IncidentFilter } from "@/features/incidents/model";
	import type { RbacCockpitState } from "@/features/rbac/cockpitModel";
	import type { RbacView } from "@/features/rbac/surfaceModel";
	import type { HealthFilter } from "@/features/resources";
	import type {
		ArgoApplicationSummary,
		HelmReleaseSummary,
		ResourceSummary,
	} from "@/lib/types";
	import type { TreeNodeId } from "@/lib/tree-nav";
	import type { PathStateDetailTab, PathStateSurfacesState } from "@/lib/path-state";
	import type { SavedWorkspace } from "@/lib/workspace-model";
	import type { WorkspaceReadContext } from "@/lib/workspaceReadContext";
	import type { RbacVerifierHandoff } from "@/features/rbac";
	import DeferredSurface from "@/components/DeferredSurface.svelte";
	import { treeNodeForResource, type WorkspaceViewMode } from "./workspaceNavigation";

	let {
		workspace,
		workspaceReadContext,
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
		workspaceReadContext: WorkspaceReadContext;
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

	const sourceReady = $derived(workspaceReadContext.sourceReady);
	const kubeconfigSourceKey = $derived(workspaceReadContext.kubeconfigSourceKey);
	const showKubeconfigSourceLabels = $derived(
		workspaceReadContext.showKubeconfigSourceLabels,
	);

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
		<DeferredSurface load={loadGitOps} label="GitOps">
			{#snippet children(GitOpsSurface)}
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
			{/snippet}
		</DeferredSurface>
	{/key}
{:else if viewMode === "helm"}
	{#key workspace.id}
		<DeferredSurface load={loadHelm} label="Helm">
			{#snippet children(HelmSurface)}
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
			{/snippet}
		</DeferredSurface>
	{/key}
{:else if viewMode === "rbac"}
	{#key `${workspace.id}:${workspace.scope.clusterContext}:${kubeconfigSourceKey ?? ""}`}
		<DeferredSurface load={loadRbac} label="RBAC">
			{#snippet children(RbacSurface)}
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
			{/snippet}
		</DeferredSurface>
	{/key}
{:else if viewMode === "incidents"}
	{#key workspace.id}
		<DeferredSurface load={loadIncidents} label="incidents">
			{#snippet children(IncidentSurface)}
				<IncidentSurface
					{workspace}
					{sourceReady}
					{kubeconfigSourceKey}
					bind:incidentFilter
					{onOpenResources}
					{onResourceInspect}
					onResourceSelect={(resource) => onResourceSelect(resource, treeNodeForResource(resource))}
				/>
			{/snippet}
		</DeferredSurface>
	{/key}
{:else if viewMode === "portForwards"}
	{#key workspace.id}
		<DeferredSurface load={loadLiveSessions} label="live sessions">
			{#snippet children(LiveSessionsSurface)}
				<LiveSessionsSurface
					{workspace}
					{sourceReady}
					{kubeconfigSourceKey}
					{showKubeconfigSourceLabels}
				/>
			{/snippet}
		</DeferredSurface>
	{/key}
{:else if viewMode === "settings"}
	<DeferredSurface load={loadSettings} label="settings">
		{#snippet children(SettingsSurface)}
			<SettingsSurface
				onBack={onCloseSettings}
				clusterContext={workspace.scope.clusterContext}
				workspaceId={workspace.id}
				kubeconfigEnvVar={workspaceReadContext.kubeconfigSourceKey}
			/>
		{/snippet}
	</DeferredSurface>
{/if}
