<script lang="ts">
	import { createQueries, createQuery, useQueryClient } from "@tanstack/svelte-query";
	import {
		countIncidentItems,
		filterIncidentItems,
		groupIncidentItems,
		type IncidentFilter,
	} from "@/features/incidents/helpers";
	import {
		buildLiveSessionReadModel,
		podExecQueryOptions,
		podExecSessionsForWorkspace,
		parseSavedPortForwardForm,
		portForwardErrorMessage,
		portForwardLocalUrl,
		portForwardQueryOptions,
		portForwardSessionsForWorkspace,
		reconnectPortForward as reconnectPortForwardLifecycle,
		savedPortForwardLabel,
		startSavedPortForward as startSavedPortForwardLifecycle,
		startSavedPortForwards as startSavedPortForwardsLifecycle,
		stopPodExec,
		stopPortForward as stopPortForwardLifecycle,
		type SavedPortForwardFormValues,
	} from "@/features/live-sessions";
	import {
		findHelmReleaseTarget,
		groupHelmReleasesByNamespace,
		helmReconciliationStatusTone,
		helmReleaseKey,
		helmStatusTone,
		sortHelmReconciliationResources,
	} from "@/features/helm/helpers";
	import {
		argoApplicationResourceNamespaces,
	} from "@/features/resources/helpers";
	import type { HealthFilter } from "@/features/resources/helpers";
	import { buildWorkspaceFetchKeys, buildWorkspaceFetchPlans } from "@/features/workspaces/query";
	import { queryKeys } from "@/lib/queryKeys";
	import {
		createTauriClient,
		detectArgoCD,
		detectFlux,
		getHelmReleaseDetails,
		getHelmReleaseReconciliation,
		getKubeconfigSources,
		listArgoApplicationSets,
		listArgoApplications,
		listArgoAppProjects,
		listFluxResources,
		listHelmReleases,
		listIncidentCockpit,
		listRbacInspection,
	} from "@/lib/tauri";
	import type {
		ArgoApplicationSetSummary,
		ArgoApplicationSummary,
		ArgoAppProjectSummary,
		FluxResourceSummary,
		HelmReconciliationResource,
		HelmReleaseReconciliation,
		HelmReleaseDetails,
		HelmReleaseSummary,
		IncidentCockpitSummary,
		PodExecSessionSummary,
		PortForwardSessionSummary,
		RbacInspectionSummary,
		ResourceSummary,
	} from "@/lib/types";
	import type { TreeNodeId } from "@/lib/tree-nav";
	import type { PathStateDetailTab, PathStateSurfacesState } from "@/lib/path-state";
	import {
		workspaceScopeContexts,
		type SavedPortForward,
		type SavedWorkspace,
	} from "@/lib/workspace-model";
	import { workspaceStore } from "@/features/workspaces/workspaceStore";
	import GitOpsSurface from "./GitOpsSurface.svelte";
	import HelmSurface from "./HelmSurface.svelte";
	import IncidentSurface from "./IncidentSurface.svelte";
	import LiveSessionsSurface from "./LiveSessionsSurface.svelte";
	import RbacSurface from "./RbacSurface.svelte";
	import SettingsSurface from "./SettingsSurface.svelte";
	import { getSettingsSnapshot, settingsStore } from "@/lib/settings-store";
	import {
		buildGitOpsRailItems,
		buildGitOpsSelections,
		buildGitOpsTable,
		gitOpsActiveRailKey as resolveGitOpsActiveRailKey,
		gitOpsSelectionKey,
		gitOpsUnavailableProvider as resolveGitOpsUnavailableProvider,
		type GitOpsSelection,
	} from "./gitOpsSurfaceModel";
	import {
		buildIncidentFilterOptions,
	} from "./incidentSurfaceModel";
	import {
		buildRbacStats,
		buildRbacTable,
		selectedRbacView,
		rbacWarningSummary,
	} from "./rbacSurfaceModel";
	import {
		type WorkspaceViewMode,
	} from "./workspaceNavigation";

	interface GitOpsData {
		argoDetected: boolean;
		apps: ArgoApplicationSummary[];
		appSets: ArgoApplicationSetSummary[];
		projects: ArgoAppProjectSummary[];
		flux: FluxResourceSummary[];
		fluxDetected: boolean;
	}

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
	const queryClient = useQueryClient();
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
	let stoppingSessionId = $state<string | null>(null);
	let copyingSessionId = $state<string | null>(null);
	let reconnectingSessionId = $state<string | null>(null);
	let startingSavedPortForwardId = $state<string | null>(null);
	let startingSavedPortForwards = $state(false);
	let savedPortForwardActionMessage = $state<string | null>(null);
	let savedPortForwardFormOpen = $state(false);
	let editingSavedPortForwardId = $state<string | null>(null);
	let savedPortForwardFormError = $state<string | null>(null);
	let savedPortForwardForm = $state<SavedPortForwardFormValues>(newSavedPortForwardForm());
	let liveSessionActionError = $state<unknown>(null);
	const autoStartSavedPortForwards = $derived(
		$settingsStore.autoStartSavedPortForwards,
	);

	const sourceQuery = createQuery(() => ({
		queryKey: ["kubeconfig-sources"],
		queryFn: () => getKubeconfigSources(client),
		staleTime: 30_000,
	}));
	const sourceReady = $derived(sourceQuery.isSuccess || sourceQuery.isError);
	const kubeconfigSourceKey = $derived(sourceQuery.data?.sourceKey);
	const showKubeconfigSourceLabels = $derived(sourceQuery.data?.showSourceLabels ?? false);
	const incidentFetchKeys = $derived(buildWorkspaceFetchKeys(workspace.scope));
	const workspaceContextKey = $derived(workspaceScopeContexts(workspace.scope).join("|"));

	const argoDetectionQuery = createQuery<boolean>(() => ({
		queryKey: queryKeys.argoDetect(workspace.scope.clusterContext, kubeconfigSourceKey),
		queryFn: () => detectArgoCD(client, workspace.scope.clusterContext, kubeconfigSourceKey),
		enabled: viewMode === "argo" && sourceReady,
		staleTime: 60_000,
	}));
	const fluxDetectionQuery = createQuery(() => ({
		queryKey: queryKeys.fluxDetect(workspace.scope.clusterContext, kubeconfigSourceKey),
		queryFn: () => detectFlux(client, workspace.scope.clusterContext, kubeconfigSourceKey),
		enabled: viewMode === "argo" && sourceReady,
		staleTime: 60_000,
	}));
	const argoAppsQuery = createQuery<ArgoApplicationSummary[]>(() => ({
		queryKey: queryKeys.argoApps(workspace.scope.clusterContext, kubeconfigSourceKey),
		queryFn: () => listArgoApplications(client, workspace.scope.clusterContext, kubeconfigSourceKey),
		enabled: viewMode === "argo" && sourceReady && argoDetectionQuery.data === true,
		staleTime: 15_000,
	}));
	const argoAppSetsQuery = createQuery<ArgoApplicationSetSummary[]>(() => ({
		queryKey: queryKeys.argoAppSets(workspace.scope.clusterContext, kubeconfigSourceKey),
		queryFn: () =>
			listArgoApplicationSets(client, workspace.scope.clusterContext, kubeconfigSourceKey),
		enabled: viewMode === "argo" && sourceReady && argoDetectionQuery.data === true,
		staleTime: 15_000,
	}));
	const argoProjectsQuery = createQuery<ArgoAppProjectSummary[]>(() => ({
		queryKey: queryKeys.argoAppProjects(workspace.scope.clusterContext, kubeconfigSourceKey),
		queryFn: () => listArgoAppProjects(client, workspace.scope.clusterContext, kubeconfigSourceKey),
		enabled: viewMode === "argo" && sourceReady && argoDetectionQuery.data === true,
		staleTime: 15_000,
	}));
	const fluxResourceQueries = createQueries(() => ({
		queries: (fluxDetectionQuery.data?.kinds ?? []).map((kind) => ({
			queryKey: queryKeys.fluxResources(
				workspace.scope.clusterContext,
				kind,
				kubeconfigSourceKey,
			),
			queryFn: () =>
				listFluxResources(client, workspace.scope.clusterContext, kind, kubeconfigSourceKey),
			enabled: viewMode === "argo" && sourceReady && fluxDetectionQuery.data?.detected === true,
			staleTime: 15_000,
		})),
	}));
	const fluxResources = $derived(
		fluxResourceQueries.flatMap((query) => (query.data as FluxResourceSummary[] | undefined) ?? []),
	);
	const argoDetectionReady = $derived(
		argoDetectionQuery.isSuccess || argoDetectionQuery.isError,
	);
	const fluxDetectionReady = $derived(
		fluxDetectionQuery.isSuccess || fluxDetectionQuery.isError,
	);
	const gitOpsProviderError = $derived(
		argoDetectionQuery.error ?? fluxDetectionQuery.error,
	);
	const gitOpsListError = $derived(
		argoAppsQuery.error ??
			argoAppSetsQuery.error ??
			argoProjectsQuery.error ??
			fluxResourceQueries.find((query) => query.error)?.error,
	);
	const gitOpsData = $derived<GitOpsData | undefined>(
		!argoDetectionReady || !fluxDetectionReady
			? undefined
			: {
					argoDetected: argoDetectionQuery.data === true,
					apps: argoAppsQuery.data ?? [],
					appSets: argoAppSetsQuery.data ?? [],
					projects: argoProjectsQuery.data ?? [],
					flux: fluxResources,
					fluxDetected: fluxDetectionQuery.data?.detected === true,
				},
	);
	const gitOpsQuery = $derived({
		data: gitOpsData,
		isPending:
			argoDetectionQuery.isPending ||
			fluxDetectionQuery.isPending ||
			(argoDetectionQuery.data === true &&
				(argoAppsQuery.isPending || argoAppSetsQuery.isPending || argoProjectsQuery.isPending)) ||
			(fluxDetectionQuery.data?.detected === true &&
				fluxResourceQueries.some((query) => query.isPending)),
		isError: false,
		error: null,
	});

	const selectedGitOpsItemKey = $derived(
		selectedGitOpsItem ? gitOpsSelectionKey(selectedGitOpsItem) : "",
	);

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

	const incidentsQuery = createQuery<IncidentCockpitSummary>(() => ({
		queryKey: queryKeys.incidentCockpit(workspaceContextKey, incidentFetchKeys, kubeconfigSourceKey),
		queryFn: () => loadIncidents(workspace, kubeconfigSourceKey),
		enabled: viewMode === "incidents" && sourceReady && incidentFetchKeys.length > 0,
		staleTime: 15_000,
	}));

	const portForwardsQuery = createQuery<PortForwardSessionSummary[]>(() =>
		portForwardQueryOptions(client, {
			enabled: viewMode === "portForwards" && sourceReady,
			refetchInterval: 2_500,
		}),
	);
	const execSessionsQuery = createQuery<PodExecSessionSummary[]>(() =>
		podExecQueryOptions(client, {
			enabled: viewMode === "portForwards" && sourceReady,
			refetchInterval: 2_500,
		}),
	);
	const liveSessionsQuery = $derived({
		isPending: portForwardsQuery.isPending || execSessionsQuery.isPending,
		isError: portForwardsQuery.isError || execSessionsQuery.isError,
		error: portForwardsQuery.error ?? execSessionsQuery.error,
	});

	const visibleIncidents = $derived(
		filterIncidentItems(incidentsQuery.data?.items ?? [], incidentFilter),
	);
	const incidentCounts = $derived(countIncidentItems(incidentsQuery.data?.items ?? []));
	const incidentFilterOptions = $derived(buildIncidentFilterOptions(incidentCounts));
	const incidentGroups = $derived(groupIncidentItems(visibleIncidents));
	const liveSessionReadModel = $derived(
		buildLiveSessionReadModel(
			portForwardSessionsForWorkspace(
				portForwardsQuery.data ?? [],
				workspace,
				kubeconfigSourceKey,
			),
			podExecSessionsForWorkspace(
				execSessionsQuery.data ?? [],
				workspace,
				kubeconfigSourceKey,
			),
		),
	);
	const visiblePortForwardSessions = $derived(liveSessionReadModel.portForwards);
	const visibleExecSessions = $derived(liveSessionReadModel.podExecSessions);
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
	const gitOpsTable = $derived(
		gitOpsQuery.data ? buildGitOpsTable(gitOpsQuery.data, selectedNode) : null,
	);
	const gitOpsUnavailableProvider = $derived(
		gitOpsQuery.data ? resolveGitOpsUnavailableProvider(gitOpsQuery.data, selectedNode) : null,
	);
	const gitOpsSelections = $derived(
		gitOpsQuery.data ? buildGitOpsSelections(gitOpsQuery.data, selectedNode) : [],
	);
	const gitOpsRailItems = $derived(
		gitOpsQuery.data ? buildGitOpsRailItems(gitOpsQuery.data) : [],
	);
	const gitOpsActiveRailKey = $derived(
		gitOpsQuery.data ? resolveGitOpsActiveRailKey(gitOpsQuery.data, selectedNode) : "",
	);

	$effect(() => {
		if (viewMode === "incidents") incidentFilter = initialIncidentFilter;
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
		if (viewMode !== "argo") {
			selectedGitOpsItem = null;
			return;
		}
		if (targetGitOpsApplication) {
			const match = gitOpsSelections.find(
				(item) => item.type === "argoApp" && item.item.name === targetGitOpsApplication,
			);
			if (match) {
				selectedGitOpsItem = match;
				onTargetGitOpsApplicationResolved?.();
				return;
			}
			if (gitOpsQuery.data) onTargetGitOpsApplicationResolved?.();
		}
		if (
			selectedGitOpsItem &&
			!gitOpsSelections.some((item) => gitOpsSelectionKey(item) === selectedGitOpsItemKey)
		) {
			selectedGitOpsItem = null;
		}
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

	async function loadIncidents(
		targetWorkspace: SavedWorkspace,
		sourceKey?: string,
	): Promise<IncidentCockpitSummary> {
		const summaries = await Promise.all(
			buildWorkspaceFetchPlans(targetWorkspace.scope).map((plan) =>
				listIncidentCockpit(client, plan.clusterContext, plan.requests, sourceKey),
			),
		);
		return {
			cluster: targetWorkspace.scope.clusterContext,
			generatedAt: new Date().toISOString(),
			requestedScope: summaries.flatMap((summary) => summary.requestedScope),
			items: summaries.flatMap((summary) => summary.items),
			warnings: summaries.flatMap((summary) =>
				summary.warnings.map((warning) => `${summary.cluster}: ${warning}`),
			),
		};
	}

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

	function portForwardSessionTitle(session: PortForwardSessionSummary): string {
		return `${session.targetKind}/${session.targetName}:${session.remotePort}`;
	}

	function portForwardSessionResolution(session: PortForwardSessionSummary): string {
		if (session.targetKind === "Service") {
			return `Resolved Pod: ${session.resolvedPodName}:${session.resolvedPodPort}`;
		}
		return `Pod: ${session.resolvedPodName}:${session.resolvedPodPort}`;
	}

	function setAutoStartSavedPortForwards(autoStart: boolean) {
		getSettingsSnapshot().setAutoStartSavedPortForwards(autoStart);
	}

	function newSavedPortForwardForm(): SavedPortForwardFormValues {
		return {
			clusterContext: workspace.scope.clusterContext,
			namespace: workspace.scope.namespaces[0] ?? "",
			serviceName: "",
			servicePort: "",
			localPort: "",
			label: "",
		};
	}

	function savedPortForwardFormFromSaved(
		portForward: SavedPortForward,
	): SavedPortForwardFormValues {
		return {
			clusterContext: portForward.clusterContext,
			namespace: portForward.namespace,
			serviceName: portForward.serviceName,
			servicePort: String(portForward.servicePort),
			localPort:
				portForward.localPort === undefined ? "" : String(portForward.localPort),
			label: portForward.label ?? "",
		};
	}

	function resetSavedPortForwardForm() {
		savedPortForwardFormOpen = false;
		editingSavedPortForwardId = null;
		savedPortForwardFormError = null;
		savedPortForwardForm = newSavedPortForwardForm();
	}

	function beginAddSavedPortForward() {
		editingSavedPortForwardId = null;
		savedPortForwardFormError = null;
		savedPortForwardForm = newSavedPortForwardForm();
		savedPortForwardFormOpen = true;
	}

	function beginEditSavedPortForward(portForward: SavedPortForward) {
		editingSavedPortForwardId = portForward.id;
		savedPortForwardFormError = null;
		savedPortForwardForm = savedPortForwardFormFromSaved(portForward);
		savedPortForwardFormOpen = true;
	}

	function setSavedPortForwardFormValue(
		key: keyof SavedPortForwardFormValues,
		value: string,
	) {
		savedPortForwardForm = { ...savedPortForwardForm, [key]: value };
	}

	function inputValue(event: Event): string {
		return (event.currentTarget as HTMLInputElement).value;
	}

	function submitSavedPortForwardForm() {
		const parsed = parseSavedPortForwardForm(savedPortForwardForm);
		if (typeof parsed === "string") {
			savedPortForwardFormError = parsed;
			return;
		}
		if (!workspaceScopeContexts(workspace.scope).includes(parsed.clusterContext)) {
			savedPortForwardFormError = "Cluster context must be in the current workspace scope.";
			return;
		}
		if (editingSavedPortForwardId) {
			workspaceStore.updateSavedPortForward(
				workspace.id,
				editingSavedPortForwardId,
				parsed,
			);
		} else {
			workspaceStore.saveSavedPortForward(workspace.id, parsed);
		}
		savedPortForwardActionMessage = editingSavedPortForwardId
			? "Saved forward updated."
			: "Saved forward added.";
		resetSavedPortForwardForm();
	}

	function deleteSavedPortForward(portForward: SavedPortForward) {
		workspaceStore.deleteSavedPortForward(workspace.id, portForward.id);
		if (editingSavedPortForwardId === portForward.id) resetSavedPortForwardForm();
		savedPortForwardActionMessage = `Deleted ${savedPortForwardLabel(portForward)}.`;
	}

	async function copyPortForwardUrl(session: PortForwardSessionSummary) {
		copyingSessionId = session.id;
		liveSessionActionError = null;
		try {
			await navigator.clipboard.writeText(portForwardLocalUrl(session));
		} catch (error) {
			liveSessionActionError = error;
		} finally {
			copyingSessionId = null;
		}
	}

	async function reconnectPortForward(session: PortForwardSessionSummary) {
		reconnectingSessionId = session.id;
		liveSessionActionError = null;
		savedPortForwardActionMessage = null;
		try {
			await reconnectPortForwardLifecycle({
				client,
				session,
				invalidateQueries: (options) => queryClient.invalidateQueries(options),
			});
			savedPortForwardActionMessage = `Reconnected ${portForwardSessionTitle(session)}.`;
		} catch (error) {
			liveSessionActionError = error;
		} finally {
			reconnectingSessionId = null;
		}
	}

	async function startSavedPortForward(portForward: SavedPortForward) {
		startingSavedPortForwardId = portForward.id;
		liveSessionActionError = null;
		savedPortForwardActionMessage = null;
		try {
			const result = await startSavedPortForwardLifecycle({
				client,
				workspaceId: workspace.id,
				portForward,
				knownSessions: portForwardsQuery.data,
				kubeconfigSource: kubeconfigSourceKey,
				updateSavedPortForward: workspaceStore.updateSavedPortForward,
				invalidateQueries: (options) => queryClient.invalidateQueries(options),
			});
			if (result.ok && result.skipped) {
				savedPortForwardActionMessage = `${savedPortForwardLabel(portForward)} already active.`;
				return;
			}
			if (!result.ok) {
				liveSessionActionError = result.error ?? "Saved port-forward failed to start.";
				return;
			}
			savedPortForwardActionMessage = `Started ${savedPortForwardLabel(portForward)}.`;
		} catch (error) {
			const message = portForwardErrorMessage(error);
			workspaceStore.updateSavedPortForward(workspace.id, portForward.id, {
				lastStatus: "error",
				lastError: message,
			});
			liveSessionActionError = error;
		} finally {
			startingSavedPortForwardId = null;
		}
	}

	async function startAllSavedPortForwards() {
		startingSavedPortForwards = true;
		liveSessionActionError = null;
		savedPortForwardActionMessage = null;
		try {
			const results = await startSavedPortForwardsLifecycle({
				client,
				workspace,
				portForwards: workspace.portForwards ?? [],
				activeSessions: portForwardsQuery.data,
				kubeconfigSource: kubeconfigSourceKey,
				updateSavedPortForward: workspaceStore.updateSavedPortForward,
				invalidateQueries: (options) => queryClient.invalidateQueries(options),
			});
			const failures = results.filter((result) => !result.ok).length;
			const conflicts = results.filter((result) => result.conflict).length;
			savedPortForwardActionMessage =
				failures > 0
					? conflicts > 0
						? `${conflicts} saved ${conflicts === 1 ? "forward has" : "forwards have"} local port conflicts.`
						: `${failures} saved ${failures === 1 ? "forward" : "forwards"} failed to start.`
				: `Started ${results.length} saved ${results.length === 1 ? "forward" : "forwards"}.`;
		} catch (error) {
			liveSessionActionError = error;
		} finally {
			startingSavedPortForwards = false;
		}
	}

	async function stopPortForwardSession(sessionId: string) {
		stoppingSessionId = sessionId;
		liveSessionActionError = null;
		try {
			await stopPortForwardLifecycle({
				client,
				sessionId,
				invalidateQueries: (options) => queryClient.invalidateQueries(options),
			});
		} catch (error) {
			liveSessionActionError = error;
		} finally {
			stoppingSessionId = null;
		}
	}

	async function stopExecSession(sessionId: string) {
		stoppingSessionId = sessionId;
		liveSessionActionError = null;
		try {
			await stopPodExec({
				client,
				sessionId,
				invalidateQueries: (options) => queryClient.invalidateQueries(options),
			});
		} catch (error) {
			liveSessionActionError = error;
		} finally {
			stoppingSessionId = null;
		}
	}

</script>

{#if viewMode === "argo"}
	<GitOpsSurface
		{gitOpsQuery}
		{gitOpsProviderError}
		{gitOpsListError}
		{gitOpsUnavailableProvider}
		{gitOpsTable}
		{gitOpsSelections}
		{gitOpsRailItems}
		{gitOpsActiveRailKey}
		bind:selectedGitOpsItem
		{selectedGitOpsItemKey}
		{openSelectedArgoApplicationResources}
		{onResourceInspect}
		{gitOpsStatusClass}
	/>
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
	<IncidentSurface
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
{:else if viewMode === "portForwards"}
	<LiveSessionsSurface
		{workspace}
		{liveSessionsQuery}
		{autoStartSavedPortForwards}
		{setAutoStartSavedPortForwards}
		{startingSavedPortForwards}
		{startAllSavedPortForwards}
		{visiblePortForwardSessions}
		{visibleExecSessions}
		{liveSessionActionError}
		{savedPortForwardActionMessage}
		{showKubeconfigSourceLabels}
		{reconnectingSessionId}
		{copyingSessionId}
		{stoppingSessionId}
		{startingSavedPortForwardId}
		{savedPortForwardFormOpen}
		{savedPortForwardForm}
		{savedPortForwardFormError}
		{editingSavedPortForwardId}
		{kubeconfigSourceKey}
		{portForwardSessionTitle}
		{portForwardSessionResolution}
		{beginAddSavedPortForward}
		{beginEditSavedPortForward}
		{resetSavedPortForwardForm}
		{submitSavedPortForwardForm}
		{deleteSavedPortForward}
		{copyPortForwardUrl}
		{reconnectPortForward}
		{startSavedPortForward}
		{stopPortForwardSession}
		{stopExecSession}
		{setSavedPortForwardFormValue}
		{inputValue}
	/>
{:else if viewMode === "settings"}
	<SettingsSurface onBack={onCloseSettings} />
{/if}
