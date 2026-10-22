<script lang="ts">
	import ArgoOperationMonitor from "@/features/gitops/ArgoOperationMonitor.svelte";
	import { markStartup } from "@/lib/startup-marks";
	import { onMount } from "svelte";
	import { createQuery } from "@tanstack/svelte-query";
	import {
		Button,
		Alert,
		AlertDescription,
		AlertTitle,
		Card,
		CardContent,
		CardDescription,
		CardHeader,
		CardTitle,
		Empty,
		EmptyDescription,
		EmptyHeader,
		EmptyTitle,
		ScrollArea,
		SidebarInset,
		SidebarProvider,
	} from "@/components/ui/svelte";
	import {
		summarizeWorkspaceScope,
		type CreateWorkspaceInput,
		type SavedWorkspace,
	} from "@/lib/workspace-model";
	import type { ArgoApplicationSummary } from "@/lib/gitops-types";
	import { buildWorkspaceReadContext } from "@/lib/workspaceReadContext";
	import {
		type DiscoveredResourceKind,
		type ResourceKindSelection,
		type ResourceSummary,
	} from "@/lib/types";
	import { nodeIdToString, type TreeNodeId } from "@/lib/tree-nav";
	import { queryKeys } from "@/lib/queryKeys";
	import {
		entryPointFromResource,
		isPinnedEntry,
		normalizeEntryPoints,
		resourceFromEntryPoint,
	} from "@/lib/workspace-entry-points";
	import {
		createTauriClient,
		listPresentCustomResourceKinds,
	} from "@/lib/tauri";
	import { kubeconfigSourcesQueryOptions } from "@/lib/kubeconfig-sources-query";
	import {
		schedulePathStateWrite,
		flushPathStateWrite,
		type PathStateDetailTab,
		type PathStateResourceDetailState,
		type PathStateSurfacesState,
		type PathStateWorkspaceSnapshot,
	} from "@/lib/path-state";
	import SavedPortForwardRestore from "@/features/live-sessions/SavedPortForwardRestore.svelte";
	import NamespaceList from "@/features/resources/NamespaceList.svelte";
	import ResourceBrowser from "@/features/resources/ResourceBrowser.svelte";
	import type { HealthFilter } from "@/features/resources/helpers";
	import { resourceKey } from "@/lib/resource-identity";
	import type { IncidentFilter } from "@/features/incidents";
	import {
		onOpenRbacVerifier,
		type RbacVerifierHandoff,
	} from "@/features/rbac/handoff";
	import WorkspaceOverview from "@/features/workspaces/WorkspaceOverview.svelte";
	import { gitOpsSelectionResource } from "@/lib/gitops-resource";
	import { workspaceStore } from "@/features/workspaces/workspaceStore";
	import AppSurfaces from "./AppSurfaces.svelte";
	import AppUsageFooter from "./AppUsageFooter.svelte";
	import CommandPalette from "./CommandPalette.svelte";
	import DetailPanelFrame from "./DetailPanelFrame.svelte";
	import ResourceInspector from "./ResourceInspector.svelte";
	import WorkspaceHeader from "./WorkspaceHeader.svelte";
	import WorkspaceSidebar from "./WorkspaceSidebar.svelte";
	import { getSettingsSnapshot, settingsStore } from "@/lib/settings-store";
	import {
		GITOPS_RESOURCE_KINDS,
		resourceBrowserAvailableKinds,
	} from "./workspaceShellModel";
	import {
		buildWorkspaceNavigationModel,
		createWorkspaceNavigation,
		navigateWorkspace,
		treeNodeForResource,
		workspacePathForWorkspace,
		workspaceNavigationSnapshot,
		type WorkspaceNavigationIntent,
		type WorkspaceNavigationState,
	} from "./workspaceNavigation";

	let {
		workspace,
		initialPathState = null,
		onPathStateConsumed = () => {},
		liveSessionCleanupMessage = null,
		onDismissLiveSessionCleanup = () => {},
		onOpenLauncher,
		onChangeClusterContext,
	}: {
		workspace: SavedWorkspace;
		initialPathState?: PathStateWorkspaceSnapshot | null;
		onPathStateConsumed?: () => void;
		liveSessionCleanupMessage?: string | null;
		onDismissLiveSessionCleanup?: () => void;
		onOpenLauncher: () => void;
		onChangeClusterContext: (workspaceId: string, input: CreateWorkspaceInput) => void;
	} = $props();

	const client = createTauriClient();
	const workspaceSourceQuery = createQuery(() => kubeconfigSourcesQueryOptions(client));
	const workspaceReadContext = $derived(
		buildWorkspaceReadContext({
			workspace,
			sources: workspaceSourceQuery.data,
			sourceSucceeded: workspaceSourceQuery.isSuccess,
			sourceFailed: workspaceSourceQuery.isError,
			sourceError: workspaceSourceQuery.error,
		}),
	);
	function initialPathSnapshotForWorkspace() {
		return workspacePathForWorkspace(workspace, initialPathState);
	}
	const initialPathSnapshot = initialPathSnapshotForWorkspace();
	function initialWorkspaceNavigation() {
		return createWorkspaceNavigation(workspace, initialPathSnapshot);
	}
	const initialNavigation = initialWorkspaceNavigation();
	const initialDetailPathState = initialPathSnapshot?.detail ?? null;
	const initialSurfacesPathState = initialPathSnapshot?.surfaces ?? null;
	let navigation = $state.raw<WorkspaceNavigationState>(initialNavigation);
	let sidebarNavigationRevision = $state(0);
	const selectedNode = $derived(navigation.selectedNode);
	const viewMode = $derived(navigation.viewMode);
	const focusedResource = $derived(navigation.focusedResource);
	const restoreTargetResource = $derived(navigation.restoreTargetResource);
	const targetHelmRelease = $derived(navigation.targetHelmRelease);
	const targetGitOpsApplication = $derived(navigation.targetGitOpsApplication);
	const resourceGitOpsFocusApplication = $derived(
		navigation.resourceGitOpsFocusApplication,
	);
	const resourceInitialSearch = $derived(navigation.resourceInitialSearch);
	const resourceInitialGitOpsFilter = $derived(navigation.resourceInitialGitOpsFilter);
	const resourceInitialHealthFilter = $derived(navigation.resourceInitialHealthFilter);
	const resourceNamespaceOverride = $derived(navigation.resourceNamespaceOverride);
	const initialIncidentFilter = $derived(navigation.initialIncidentFilter);
	let expandedSections = $state<string[]>(
		initialPathSnapshot?.expandedSections ??
			(initialNavigation.selectedNode
				? [nodeIdToString(initialNavigation.selectedNode)]
				: []),
	);
	let commandOpen = $state(false);
	let navigationOpen = $state(false);
	let resourceDetailPathState = $state<PathStateResourceDetailState | null>(
		initialDetailPathState,
	);
	let surfacesPathState = $state<PathStateSurfacesState | null>(
		initialSurfacesPathState,
	);
	let rbacVerifierHandoff = $state<RbacVerifierHandoff | undefined>();
	let rbacVerifierReturnNavigation = $state<WorkspaceNavigationState | null>(null);
	let rbacVerifierReturnLabel = $state<string | undefined>();
	const kubeconfigSourceKey = $derived($settingsStore.kubeconfigSourceKey);
	const showCustomResources = $derived($settingsStore.showCustomResources);

	const navigationWorkspaceName = $derived(workspace.name);
	const navigationWorkspaceScope = $derived(workspace.scope);
	const navigationModel = $derived(
		buildWorkspaceNavigationModel(
			{ name: navigationWorkspaceName, scope: navigationWorkspaceScope },
			{ selectedNode, viewMode, resourceNamespaceOverride },
		),
	);
	const title = $derived(navigationModel.title);
	const placeholder = $derived(navigationModel.placeholder);
	const scopeSummary = $derived(summarizeWorkspaceScope(workspace.scope));
	const isOverview = $derived(navigationModel.isOverview);
	const isNamespaceList = $derived(navigationModel.isNamespaceList);
	const resourceBrowserScope = $derived(navigationModel.resourceBrowserScope);
	const resourceBrowserNamespaces = $derived(resourceBrowserScope.namespaces);
	const fixedResourceKindScope = $derived(
		selectedNode?.type === "kind",
	);
	const workspaceCustomResourcePrewarmQuery = createQuery<DiscoveredResourceKind[]>(() => ({
		queryKey: queryKeys.presentCustomResourceKinds(
			workspace.scope.clusterContext,
			workspace.scope.namespaces,
			kubeconfigSourceKey,
		),
		queryFn: () =>
			listPresentCustomResourceKinds(
				client,
				workspace.scope.clusterContext,
			workspace.scope.namespaces,
			kubeconfigSourceKey,
		),
		enabled: showCustomResources && workspaceReadContext.sourceReady && Boolean(workspace.scope.clusterContext),
		staleTime: 30_000,
		retry: false,
	}));
	const includePresentCustomResources = $derived(
		resourceBrowserScope.canQuery &&
			(resourceGitOpsFocusApplication !== null ||
				resourceNamespaceOverride !== null ||
				selectedNode?.type === "namespace"),
	);
	const presentCustomResourceKindsQuery = createQuery<DiscoveredResourceKind[]>(() => ({
		queryKey: queryKeys.presentCustomResourceKinds(
			workspace.scope.clusterContext,
			resourceBrowserNamespaces,
			kubeconfigSourceKey,
		),
		queryFn: () =>
			listPresentCustomResourceKinds(
				client,
				workspace.scope.clusterContext,
				resourceBrowserNamespaces,
				kubeconfigSourceKey,
		),
		enabled:
			showCustomResources &&
			workspaceReadContext.sourceReady &&
			Boolean(workspace.scope.clusterContext) &&
			includePresentCustomResources,
		staleTime: 30_000,
		retry: false,
	}));
	const presentCustomResourceKinds = $derived(
		!showCustomResources
			? []
			: includePresentCustomResources
				? (presentCustomResourceKindsQuery.data ?? workspaceCustomResourcePrewarmQuery.data ?? [])
				: (workspaceCustomResourcePrewarmQuery.data ?? []),
	);
	const customResourcesStatus = $derived(
		!showCustomResources
			? "Custom Resources off"
			: workspaceCustomResourcePrewarmQuery.isError ||
				  (includePresentCustomResources && presentCustomResourceKindsQuery.isError)
				? "Custom Resources unavailable"
				: workspaceCustomResourcePrewarmQuery.isPending ||
					  workspaceCustomResourcePrewarmQuery.isFetching ||
					  (includePresentCustomResources &&
							(presentCustomResourceKindsQuery.isPending ||
								presentCustomResourceKindsQuery.isFetching))
					? "Loading Custom Resources"
					: presentCustomResourceKinds.length > 0
						? "Custom Resources added"
						: null,
	);
	const resourceBrowserInitialKinds = $derived<ResourceKindSelection[]>(
		resourceGitOpsFocusApplication
			? [...GITOPS_RESOURCE_KINDS]
			: resourceBrowserScope.kinds,
	);
	const resourceBrowserKinds = $derived<ResourceKindSelection[]>(
		resourceBrowserAvailableKinds(
			resourceBrowserInitialKinds,
			presentCustomResourceKinds,
			fixedResourceKindScope,
		),
	);
	const pinnedResourceKeys = $derived(
		normalizeEntryPoints(workspace.entryPoints).pinned.flatMap((entry) => {
			const resource = resourceFromEntryPoint(entry);
			return resource ? [resourceKey(resource)] : [];
		}),
	);
	const focusedResourcePinned = $derived(
		focusedResource
			? isPinnedEntry(workspace.entryPoints, entryPointFromResource(focusedResource, ""))
			: false,
	);
	const resourceInspectorOpen = $derived(focusedResource !== null);
	const resourceInspectorSizeKey = $derived(viewMode === "argo" ? "gitops" : "resource");
	const resourceInspectorDefaultSize = $derived(viewMode === "argo" ? 30 : 40);
	const resourceInspectorMinSize = $derived(viewMode === "argo" ? 25 : 33);

	onMount(() => {
		markStartup("workspace-ready");
		if (initialPathSnapshot) onPathStateConsumed();
		const stopRbacHandoff = onOpenRbacVerifier((handoff) => {
			rbacVerifierReturnNavigation = { ...navigation };
			rbacVerifierReturnLabel = `Return to ${title}`;
			rbacVerifierHandoff = handoff;
			applyWorkspaceNavigation({
				type: "selectNode",
				node: { type: "kind", section: "rbac", kind: "Bindings" },
			});
		});
		const closeNarrowNavigation = () => {
			if (window.matchMedia("(min-width: 80rem)").matches) navigationOpen = false;
		};
		window.addEventListener("resize", closeNarrowNavigation);
		window.addEventListener("pagehide", flushPathStateWrite);
		window.addEventListener("beforeunload", flushPathStateWrite);
		const flushWhenHidden = () => {
			if (document.visibilityState === "hidden") flushPathStateWrite();
		};
		document.addEventListener("visibilitychange", flushWhenHidden);
		return () => {
			flushPathStateWrite();
			stopRbacHandoff();
			window.removeEventListener("resize", closeNarrowNavigation);
			window.removeEventListener("pagehide", flushPathStateWrite);
			window.removeEventListener("beforeunload", flushPathStateWrite);
			document.removeEventListener("visibilitychange", flushWhenHidden);
		};
	});

	function returnFromRbacVerifier() {
		if (rbacVerifierReturnNavigation) navigation = rbacVerifierReturnNavigation;
		rbacVerifierHandoff = undefined;
		rbacVerifierReturnNavigation = null;
		rbacVerifierReturnLabel = undefined;
	}

	function applyWorkspaceNavigation(intent: WorkspaceNavigationIntent, revealSidebar = true) {
		navigation = navigateWorkspace(navigation, intent);
		if (revealSidebar && ["selectNode", "selectResource", "openResources", "openHelmRelease", "openArgo", "openIncidents", "openPortForwards"].includes(intent.type)) {
			sidebarNavigationRevision += 1;
		}
	}

	function openLauncher() {
		applyWorkspaceNavigation({ type: "openLauncher" });
		onOpenLauncher();
	}

	function changeClusterContext(clusterContext: string) {
		if (clusterContext === workspace.scope.clusterContext) return;
		onChangeClusterContext(workspace.id, {
			name: workspace.name,
			clusterContext,
			clusterContexts: [clusterContext],
			namespaces: [],
			kinds: workspace.scope.kinds,
			shortcutPreferences: workspace.scope.shortcutPreferences,
		});
	}

	function currentPathState(): PathStateWorkspaceSnapshot {
		return workspaceNavigationSnapshot(navigation, {
			workspaceId: workspace.id,
			expandedSections,
			detail: resourceDetailPathState,
			surfaces: surfacesPathState,
		});
	}

	$effect(() => {
		schedulePathStateWrite({
			version: 1,
			runtime: "svelte",
			launcherView: "workspaces",
			workspace: currentPathState(),
		});
	});

	function openSettings() {
		applyWorkspaceNavigation({ type: "openSettings" });
	}

	function closeSettings() {
		applyWorkspaceNavigation({ type: "closeSettings" });
	}

	function openResources(
		namespace?: string | string[],
		initialSearch = "",
		initialGitOpsFilter = workspace.scope.gitOpsFilter ?? workspace.scope.argoAppFilter ?? "",
		initialHealthFilter: HealthFilter = "all",
		gitOpsFocusApplication: ArgoApplicationSummary | null = null,
	) {
		if (namespace && !Array.isArray(namespace)) {
			workspaceStore.recordRecentNamespace(
				workspace.id,
				workspace.scope.clusterContext,
				namespace,
			);
		}
		if (gitOpsFocusApplication) {
			workspaceStore.recordRecentApplication(
				workspace.id,
				gitOpsFocusApplication.cluster,
				gitOpsFocusApplication.name,
				gitOpsFocusApplication.namespace,
			);
		}
		applyWorkspaceNavigation({
			type: "openResources",
			namespaces: namespace,
			search: initialSearch,
			gitOpsFilter: initialGitOpsFilter,
			healthFilter: initialHealthFilter,
			gitOpsFocusApplication,
		});
	}

	function openHelmReleaseFromResource(releaseName: string, namespace?: string | null) {
		applyWorkspaceNavigation({
			type: "openHelmRelease",
			name: releaseName,
			namespace,
		});
	}

	function clearTargetHelmRelease() {
		applyWorkspaceNavigation({ type: "clearHelmTarget" });
	}

	function clearTargetGitOpsApplication() {
		applyWorkspaceNavigation({ type: "clearGitOpsTarget" });
	}

	function openArgo(argoApp?: string, namespace?: string) {
		if (argoApp) {
			workspaceStore.recordRecentApplication(
				workspace.id,
				workspace.scope.clusterContext,
				argoApp,
				namespace,
			);
		}
		applyWorkspaceNavigation({ type: "openArgo", application: argoApp });
	}

	function openIncidents(filter: IncidentFilter = "all") {
		applyWorkspaceNavigation({ type: "openIncidents", filter });
	}

	function openPortForwards() {
		applyWorkspaceNavigation({ type: "openPortForwards" });
	}

	function selectNode(nodeId: TreeNodeId, revealSidebar = true) {
		navigationOpen = false;
		if (nodeId.type === "namespace" && nodeId.namespace) {
			workspaceStore.recordRecentNamespace(
				workspace.id,
				workspace.scope.clusterContext,
				nodeId.namespace,
			);
		}
		applyWorkspaceNavigation({ type: "selectNode", node: nodeId }, revealSidebar);
	}

	function selectResource(resource: ResourceSummary, nodeId: TreeNodeId) {
		workspaceStore.recordRecentResource(workspace.id, resource);
		applyWorkspaceNavigation({ type: "selectResource", resource, node: nodeId });
	}

	function detailPathStateForTab(activeTab: PathStateDetailTab): PathStateResourceDetailState {
		const settings = getSettingsSnapshot();
		return {
			activeTab,
			metadataLabelsExpanded: resourceDetailPathState?.metadataLabelsExpanded ?? false,
			metadataAnnotationsExpanded: resourceDetailPathState?.metadataAnnotationsExpanded ?? false,
			selectedContainer: resourceDetailPathState?.selectedContainer ?? "",
			logFilter: resourceDetailPathState?.logFilter ?? "",
			logWrapLines: resourceDetailPathState?.logWrapLines ?? true,
			logLatestFirst: resourceDetailPathState?.logLatestFirst ?? false,
			yamlViewMode: resourceDetailPathState?.yamlViewMode ?? settings.yamlViewModeDefault,
			yamlEncoding: resourceDetailPathState?.yamlEncoding ?? settings.yamlEncodingDefault,
			yamlShowFullDiff: resourceDetailPathState?.yamlShowFullDiff ?? false,
		};
	}

	function inspectResource(resource: ResourceSummary, detailTab?: PathStateDetailTab) {
		resourceDetailPathState = detailTab ? detailPathStateForTab(detailTab) : null;
		workspaceStore.recordRecentResource(workspace.id, resource);
		applyWorkspaceNavigation({ type: "inspectResource", resource });
	}

	function focusResource(
		resource: ResourceSummary,
		source: "explicit" | "restore" = "explicit",
	) {
		if (source === "explicit") {
			workspaceStore.recordRecentResource(workspace.id, resource);
		}
		applyWorkspaceNavigation({ type: "focusResource", resource });
	}

	function togglePinnedResource(resource: ResourceSummary) {
		workspaceStore.togglePinnedResource(workspace.id, resource);
	}

	function openEntryPoint(resource: ResourceSummary) {
		workspaceStore.recordRecentResource(workspace.id, resource);
		applyWorkspaceNavigation({
			type: "selectResource",
			resource,
			node: treeNodeForResource(resource),
		});
	}

</script>

<ArgoOperationMonitor context={workspaceReadContext} />

<SidebarProvider class="h-screen overflow-hidden bg-background text-foreground xl:gap-2 xl:p-2">
	<WorkspaceSidebar
		{workspace}
		{workspaceReadContext}
		{selectedNode}
		{sidebarNavigationRevision}
		bind:expandedSections
		bind:navigationOpen
		onNodeSelect={(id) => selectNode(id, false)}
	/>

	<SidebarInset
		class="flex h-full min-w-0 flex-col overflow-hidden xl:rounded-xl xl:border xl:border-sidebar-border xl:bg-background"
	>
		<WorkspaceHeader
			clusterContext={workspace.scope.clusterContext}
			locationLabel={resourceBrowserScope.canQuery ? "Resources" : title}
			focusApplicationName={resourceGitOpsFocusApplication?.name}
			onOpenNavigation={() => (navigationOpen = true)}
			onClusterChange={changeClusterContext}
			onOpenPortForwards={openPortForwards}
			onOpenLauncher={openLauncher}
			onOpenSettings={openSettings}
			onOpenSearch={() => (commandOpen = true)}
		/>

		{#if liveSessionCleanupMessage}
			<Alert class="shrink-0 rounded-none border-x-0 border-t-0">
				<AlertTitle>Live sessions updated</AlertTitle>
				<AlertDescription class="flex flex-wrap items-center justify-between gap-3">
					<span>{liveSessionCleanupMessage}</span>
					<Button type="button" size="sm" variant="ghost" onclick={onDismissLiveSessionCleanup}>
						Dismiss
					</Button>
				</AlertDescription>
			</Alert>
		{/if}

		<SavedPortForwardRestore {client} {workspace} onReview={openPortForwards} />

		<main class="flex min-h-0 flex-1 overflow-hidden">
			<DetailPanelFrame
				detailOpen={resourceInspectorOpen}
				sizeKey={resourceInspectorSizeKey}
				detailDefaultSize={resourceInspectorDefaultSize}
				detailMinSize={resourceInspectorMinSize}
			>
				{#snippet detailPanel()}
					{#if focusedResource}
						<ResourceInspector
							{client}
							resource={focusedResource}
							pinned={focusedResourcePinned}
							argoApplicationSummary={resourceGitOpsFocusApplication}
							{workspaceReadContext}
							initialPathState={resourceDetailPathState}
							onPinToggle={() => togglePinnedResource(focusedResource)}
							onClose={() => applyWorkspaceNavigation({ type: "clearResource" })}
							onOpenHelmRelease={openHelmReleaseFromResource}
							onPathStateChange={(state) => (resourceDetailPathState = state)}
						/>
					{/if}
				{/snippet}
			<ScrollArea class="h-full">
				{#if isOverview}
					<WorkspaceOverview
						{workspace}
						{workspaceReadContext}
						onOpenResources={openResources}
						onOpenResource={openEntryPoint}
						onReconcileEntryPoints={(resources, coverage) =>
							workspaceStore.reconcileEntryPoints(workspace.id, resources, coverage)}
						onOpenArgo={openArgo}
						onOpenIncidents={openIncidents}
						onOpenPortForwards={openPortForwards}
						onOpenLauncher={openLauncher}
					/>
				{:else if isNamespaceList}
					<section class="mx-auto flex w-full max-w-5xl flex-col gap-4 p-4 md:p-6">
						<Card size="sm" elevation="flat">
							<CardHeader>
								<CardTitle>{title}</CardTitle>
								<CardDescription>{scopeSummary}</CardDescription>
							</CardHeader>
							<CardContent>
								<NamespaceList {workspaceReadContext} />
							</CardContent>
						</Card>
					</section>
				{:else if resourceBrowserScope.canQuery}
					<section class="flex h-full min-h-0 w-full flex-col gap-3 p-3 xl:p-4">
						<ResourceBrowser
							{workspaceReadContext}
							initialNamespaces={resourceBrowserNamespaces}
							initialKinds={resourceBrowserInitialKinds}
							availableKinds={resourceBrowserKinds}
							kindScopeLocked={fixedResourceKindScope}
							customResourcesEnabled={showCustomResources}
							{customResourcesStatus}
							initialSearch={resourceInitialSearch}
							initialGitOpsFilter={resourceInitialGitOpsFilter}
							initialHealthFilter={resourceInitialHealthFilter}
							gitOpsFocusApplication={resourceGitOpsFocusApplication}
							targetResource={restoreTargetResource ?? focusedResource}
							selectedResource={focusedResource}
							{title}
							initialPathState={navigation.resourceBrowserPathState}
							{pinnedResourceKeys}
							onPathStateChange={(pathState) =>
								applyWorkspaceNavigation({ type: "updateResourceBrowserPath", pathState })}
							onResourceSelect={focusResource}
							onResourcePinToggle={togglePinnedResource}
							onResourceClose={() =>
								applyWorkspaceNavigation({ type: "clearResource" })}
							onArgoApplicationInspect={(app) =>
								inspectResource(gitOpsSelectionResource({ type: "argoApp", item: app }), "argo")}
						/>
					</section>
				{:else if viewMode === "argo" || viewMode === "helm" || viewMode === "rbac" || viewMode === "incidents" || viewMode === "portForwards" || viewMode === "settings"}
					<AppSurfaces
						{workspace}
						{workspaceReadContext}
						{viewMode}
						{selectedNode}
						{targetHelmRelease}
						{targetGitOpsApplication}
						{initialIncidentFilter}
						initialPathState={initialSurfacesPathState}
						onOpenResources={openResources}
						onResourceInspect={inspectResource}
						onResourceSelect={selectResource}
						onTargetHelmReleaseResolved={clearTargetHelmRelease}
						onTargetGitOpsApplicationResolved={clearTargetGitOpsApplication}
						onPathStateChange={(state) => (surfacesPathState = state)}
						rbacVerifierHandoff={rbacVerifierHandoff}
						onRbacVerifierHandoffConsumed={() => (rbacVerifierHandoff = undefined)}
						onRbacViewChange={(rbacView) =>
							selectNode({ type: "kind", section: "rbac", kind: rbacView })}
						onRbacVerifierReturn={rbacVerifierReturnNavigation ? returnFromRbacVerifier : undefined}
						rbacVerifierReturnLabel={rbacVerifierReturnLabel}
						onCloseSettings={closeSettings}
					/>
				{:else}
					<section class="mx-auto flex w-full max-w-5xl flex-col gap-4 p-4 md:p-6">
						<Empty class="min-h-52 border border-dashed bg-surface-1/50">
							<EmptyHeader>
								<EmptyTitle>{title}</EmptyTitle>
								<EmptyDescription>{placeholder}</EmptyDescription>
							</EmptyHeader>
						</Empty>
					</section>
				{/if}
			</ScrollArea>
			</DetailPanelFrame>
		</main>
		<AppUsageFooter />
	</SidebarInset>
</SidebarProvider>

<CommandPalette
	bind:open={commandOpen}
	{workspace}
	{workspaceReadContext}
	{presentCustomResourceKinds}
	onNodeSelect={selectNode}
	onResourceSelect={selectResource}
	onOpenLauncher={openLauncher}
	onOpenSettings={openSettings}
/>
