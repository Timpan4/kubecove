<script lang="ts">
	import { createQuery } from "@tanstack/svelte-query";
	import { GitOpsSurface, type GitOpsSelection } from "@/features/gitops";
	import { IncidentSurface, type IncidentFilter } from "@/features/incidents";
	import { LiveSessionsSurface } from "@/features/live-sessions";
	import {
		findHelmReleaseTarget,
		groupHelmReleasesByNamespace,
		helmReconciliationStatusTone,
		helmReleaseKey,
		helmStatusTone,
		sortHelmReconciliationResources,
	} from "@/features/helm/helpers";
	import type { HealthFilter } from "@/features/resources";
	import { queryKeys } from "@/lib/queryKeys";
	import {
		createTauriClient,
		getHelmReleaseDetails,
		getHelmReleaseReconciliation,
		getKubeconfigSources,
		listHelmReleases,
		listRbacInspection,
	} from "@/lib/tauri";
	import type {
		ArgoApplicationSummary,
		HelmReconciliationResource,
		HelmReleaseReconciliation,
		HelmReleaseDetails,
		HelmReleaseSummary,
		RbacInspectionSummary,
		ResourceSummary,
	} from "@/lib/types";
	import type { TreeNodeId } from "@/lib/tree-nav";
	import type { PathStateDetailTab, PathStateSurfacesState } from "@/lib/path-state";
	import type { SavedWorkspace } from "@/lib/workspace-model";
	import HelmSurface from "./HelmSurface.svelte";
	import RbacSurface from "./RbacSurface.svelte";
	import SettingsSurface from "./SettingsSurface.svelte";
	import {
		buildRbacStats,
		buildRbacTable,
		selectedRbacView,
		rbacWarningSummary,
	} from "./rbacSurfaceModel";
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

	const helmQuery = createQuery<HelmReleaseSummary[]>(() => ({
		queryKey: queryKeys.helmReleases(workspace.scope.clusterContext, kubeconfigSourceKey),
		queryFn: () => listHelmReleases(client, workspace.scope.clusterContext, kubeconfigSourceKey),
		enabled: viewMode === "helm" && sourceReady,
		staleTime: 15_000,
	}));

	const selectedHelmReleaseKey = $derived(
		selectedHelmRelease ? helmReleaseKey(selectedHelmRelease) : "",
	);

	const helmDetailsQuery = createQuery<HelmReleaseDetails>(() => ({
		queryKey: selectedHelmRelease
			? queryKeys.helmReleaseDetails(
					selectedHelmRelease.cluster,
					selectedHelmRelease.namespace,
					selectedHelmRelease.storageKind,
					selectedHelmRelease.storageName,
					kubeconfigSourceKey,
					"kubectl",
					"yaml",
				)
			: ["helm-release-details", "idle"],
		queryFn: () =>
			getHelmReleaseDetails(
				client,
				selectedHelmRelease as HelmReleaseSummary,
				kubeconfigSourceKey,
				"kubectl",
				"yaml",
			),
		enabled: viewMode === "helm" && sourceReady && Boolean(selectedHelmRelease),
		staleTime: 15_000,
	}));

	const helmReconciliationQuery = createQuery<HelmReleaseReconciliation>(() => ({
		queryKey: selectedHelmRelease
			? queryKeys.helmReleaseReconciliation(
					selectedHelmRelease.cluster,
					selectedHelmRelease.namespace,
					selectedHelmRelease.storageKind,
					selectedHelmRelease.storageName,
					kubeconfigSourceKey,
				)
			: ["helm-release-reconciliation", "idle"],
		queryFn: () =>
			getHelmReleaseReconciliation(
				client,
				selectedHelmRelease as HelmReleaseSummary,
				kubeconfigSourceKey,
			),
		enabled: viewMode === "helm" && sourceReady && Boolean(selectedHelmRelease),
		staleTime: 30_000,
	}));

	const rbacQuery = createQuery<RbacInspectionSummary>(() => ({
		queryKey: queryKeys.rbacInspection(
			workspace.scope.clusterContext,
			workspace.scope.namespaces,
			kubeconfigSourceKey,
		),
		queryFn: () =>
			listRbacInspection(
				client,
				workspace.scope.clusterContext,
				workspace.scope.namespaces,
				kubeconfigSourceKey,
			),
		enabled: viewMode === "rbac" && sourceReady,
		staleTime: 30_000,
	}));

	const rbacView = $derived(selectedRbacView(selectedNode));
	const rbacTable = $derived(
		rbacQuery.data ? buildRbacTable(rbacQuery.data, rbacView) : null,
	);
	const rbacStats = $derived(rbacQuery.data ? buildRbacStats(rbacQuery.data) : []);
	const filteredHelmReleases = $derived(filterHelmReleases(helmQuery.data ?? [], helmSearch));
	const groupedHelmReleases = $derived(groupHelmReleasesByNamespace(filteredHelmReleases));
	const helmReconciliationRows = $derived(
		sortHelmReconciliationResources(helmReconciliationQuery.data?.resources ?? []),
	);
	$effect(() => {
		if (viewMode === "incidents") incidentFilter = initialIncidentFilter;
	});
	$effect(() => {
		if (viewMode !== "argo") selectedGitOpsItem = null;
	});

	$effect(() => {
		onPathStateChange({
			incidentFilter,
			helmSearch,
			selectedHelmRelease: selectedHelmRelease
				? { name: selectedHelmRelease.name, namespace: selectedHelmRelease.namespace }
				: null,
			selectedGitOpsApplication:
				selectedGitOpsItem?.type === "argoApp" ? selectedGitOpsItem.item.name : null,
		});
	});

	$effect(() => {
		if (viewMode !== "helm") {
			selectedHelmRelease = null;
			return;
		}
		if (targetHelmRelease && helmQuery.data) {
			const match = findHelmReleaseTarget(helmQuery.data, targetHelmRelease);
			if (match) {
				selectedHelmRelease = match;
				onTargetHelmReleaseResolved?.();
				return;
			}
		}
		if (
			selectedHelmRelease &&
			helmQuery.data &&
			!helmQuery.data.some((release) => helmReleaseKey(release) === selectedHelmReleaseKey)
		) {
			selectedHelmRelease = null;
		}
	});

	function filterHelmReleases(
		releases: HelmReleaseSummary[],
		search: string,
	): HelmReleaseSummary[] {
		const term = search.trim().toLowerCase();
		if (!term) return releases;
		return releases.filter((release) =>
			[
				release.name,
				release.namespace,
				release.chart ?? "",
				release.appVersion ?? "",
				release.status ?? "",
				release.storageKind,
				release.storageName,
			]
				.join(" ")
				.toLowerCase()
				.includes(term),
		);
	}

	function helmStatusVariant(status: string | undefined) {
		return helmStatusTone(status) === "error" ? "destructive" : "outline";
	}

	function helmReconciliationSource(resource: HelmReconciliationResource): string {
		if (resource.inManifest && resource.liveResource) return "Manifest + live";
		if (resource.inManifest) return "Manifest";
		if (resource.liveResource) return "Live label";
		return "-";
	}

	function helmReconciliationClass(status: HelmReconciliationResource["status"]) {
		const tone = helmReconciliationStatusTone(status);
		if (tone === "success") return "border-emerald-500/40 bg-emerald-500/10 text-emerald-700 dark:text-emerald-300";
		if (tone === "warning") return "border-amber-500/40 bg-amber-500/10 text-amber-700 dark:text-amber-300";
		if (tone === "error") return "border-destructive/40 bg-destructive/10 text-destructive";
		return "";
	}


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
	<HelmSurface
		{helmQuery}
		{groupedHelmReleases}
		{filteredHelmReleases}
		bind:helmSearch
		bind:selectedHelmRelease
		{selectedHelmReleaseKey}
		{helmDetailsQuery}
		{helmReconciliationQuery}
		{helmReconciliationRows}
		{onOpenResources}
		{helmStatusVariant}
		{helmReconciliationClass}
		{helmReconciliationSource}
	/>
{:else if viewMode === "rbac"}
	<RbacSurface {rbacQuery} {rbacStats} {rbacTable} {rbacView} {rbacWarningSummary} />
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
