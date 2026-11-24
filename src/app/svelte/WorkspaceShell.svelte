<script lang="ts">
	import { cancelledQueryRetry } from "@/lib/query-retry";
	import { loadingDelay } from "@/lib/loading-delay.svelte";
	import ArgoAutoReconnect from "@/features/gitops/ArgoAutoReconnect.svelte";
	import ArgoOperationMonitor from "@/features/gitops/ArgoOperationMonitor.svelte";
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
	import { buildWorkspaceReadContext } from "@/lib/workspaceReadContext";
	import {
		type DiscoveredResourceKind,
		type ResourceKindSelection,
	} from "@/lib/types";
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
	import type { PathStateWorkspaceSnapshot } from "@/lib/path-state";
	import SavedPortForwardRestore from "@/features/live-sessions/SavedPortForwardRestore.svelte";
	import { canLeaveYamlDraft } from "@/features/resource-detail/yamlDraftGuard.svelte";
	import YamlDraftNotice from "@/features/resource-detail/YamlDraftNotice.svelte";
	import NamespaceList from "@/features/resources/NamespaceList.svelte";
	import ResourceBrowser from "@/features/resources/ResourceBrowser.svelte";
	import { resourceKey } from "@/lib/resource-identity";
	import WorkspaceOverview from "@/features/workspaces/WorkspaceOverview.svelte";
	import { gitOpsSelectionResource } from "@/lib/gitops-resource";
	import { workspaceStore } from "@/features/workspaces/workspaceStore";
	import AppSurfaces from "./AppSurfaces.svelte";
	import { USE_BROWSER_DEV_MOCKS } from "@/lib/build-env";
	import { browserMockSurfaces } from "./browserMockSurfaces";
	import { remoteNotice } from "@/lib/remote-notice";
	import { desktopRemoteConnection } from "@/lib/remote-runtime";
	import AppUsageFooter from "./AppUsageFooter.svelte";
	import CommandPalette from "./CommandPalette.svelte";
	import DetailPanelFrame from "./DetailPanelFrame.svelte";
	import ResourceInspector from "./ResourceInspector.svelte";
	import WorkspaceHeader from "./WorkspaceHeader.svelte";
	import WorkspaceSidebar from "./WorkspaceSidebar.svelte";
	import { settingsStore } from "@/lib/settings-store";
	import {
		GITOPS_RESOURCE_KINDS,
		resourceBrowserAvailableKinds,
	} from "./workspaceShellModel";
	import { buildWorkspaceNavigationModel } from "./workspaceNavigation";
	import { workspaceNavigationController } from "./workspaceNavigationController.svelte";

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
	const controller = workspaceNavigationController({
		getWorkspace: () => workspace,
		initialPathState: initialSnapshot(),
		onPathStateConsumed: () => onPathStateConsumed(),
		onOpenLauncher: () => onOpenLauncher(),
		onChangeClusterContext: (id, input) => onChangeClusterContext(id, input),
		currentTitle: () => title,
	});
	function initialSnapshot() { return initialPathState; }
	const { returnFromRbacVerifier, applyWorkspaceNavigation, openLauncher, changeClusterContext, openSettings, closeSettings, openResources, openHelmReleaseFromResource, clearTargetHelmRelease, clearTargetGitOpsApplication, openArgo, openIncidents, openPortForwards, selectNode, selectResource, inspectResource, focusResource, togglePinnedResource, openEntryPoint } = controller;
	const selectedNode = $derived(controller.navigation.selectedNode);
	const viewMode = $derived(controller.navigation.viewMode);
	const focusedResource = $derived(controller.navigation.focusedResource);
	const restoreTargetResource = $derived(controller.navigation.restoreTargetResource);
	const targetHelmRelease = $derived(controller.navigation.targetHelmRelease);
	const helmContext = $derived(controller.navigation.helmContext);
	const targetGitOpsApplication = $derived(controller.navigation.targetGitOpsApplication);
	const resourceGitOpsFocusApplication = $derived(
		controller.navigation.resourceGitOpsFocusApplication,
	);
	const resourceInitialSearch = $derived(controller.navigation.resourceInitialSearch);
	const resourceInitialGitOpsFilter = $derived(controller.navigation.resourceInitialGitOpsFilter);
	const resourceInitialHealthFilter = $derived(controller.navigation.resourceInitialHealthFilter);
	const resourceNamespaceOverride = $derived(controller.navigation.resourceNamespaceOverride);
	const initialIncidentFilter = $derived(controller.navigation.initialIncidentFilter);
	let commandOpen = $state(false);
	const kubeconfigSourceKey = $derived($settingsStore.kubeconfigSourceKey);
	const showCustomResources = $derived($settingsStore.showCustomResources);
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
		retry: cancelledQueryRetry,
	}));

	const navigationWorkspaceName = $derived(workspace.name);
	const navigationWorkspaceScope = $derived(workspace.scope);
	const navigationModel = $derived(
		buildWorkspaceNavigationModel(
			{ name: navigationWorkspaceName, scope: navigationWorkspaceScope },
			{ selectedNode, viewMode, resourceNamespaceOverride },
			// The workspace-wide read, as the Custom Resources section spans every workspace namespace.
			showCustomResources ? (workspaceCustomResourcePrewarmQuery.data ?? []) : [],
		),
	);
	const title = $derived(navigationModel.title);
	const placeholder = $derived(navigationModel.placeholder);
	const scopeSummary = $derived(summarizeWorkspaceScope(workspace.scope));
	const isOverview = $derived(navigationModel.isOverview);
	const remoteNoticeShown = $derived(USE_BROWSER_DEV_MOCKS && viewMode !== "settings" && $desktopRemoteConnection !== null && remoteNotice($desktopRemoteConnection) !== null);
	const isNamespaceList = $derived(navigationModel.isNamespaceList);
	const resourceBrowserScope = $derived(navigationModel.resourceBrowserScope);
	const resourceBrowserNamespaces = $derived(resourceBrowserScope.namespaces);
	const fixedResourceKindScope = $derived(
		selectedNode?.type === "kind",
	);
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
		retry: cancelledQueryRetry,
	}));
	const presentCustomResourceKinds = $derived(
		!showCustomResources
			? []
			: includePresentCustomResources
				? (presentCustomResourceKindsQuery.data ?? workspaceCustomResourcePrewarmQuery.data ?? [])
				: (workspaceCustomResourcePrewarmQuery.data ?? []),
	);
	const customResourcesLoading = loadingDelay(
		() =>
			workspaceCustomResourcePrewarmQuery.isPending ||
			workspaceCustomResourcePrewarmQuery.isFetching ||
			(includePresentCustomResources &&
				(presentCustomResourceKindsQuery.isPending || presentCustomResourceKindsQuery.isFetching)),
	);
	const customResourcesStatus = $derived(
		!showCustomResources
			? "Custom Resources off"
			: workspaceCustomResourcePrewarmQuery.isError ||
				  (includePresentCustomResources && presentCustomResourceKindsQuery.isError)
				? "Custom Resources unavailable"
				: customResourcesLoading.shown
					? "Loading Custom Resources"
					: customResourcesLoading.active
						? null
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

</script>

<ArgoOperationMonitor context={workspaceReadContext} />
<ArgoAutoReconnect context={workspaceReadContext} />

<SidebarProvider class="h-screen overflow-hidden bg-background text-foreground xl:gap-2 xl:p-2">
	<WorkspaceSidebar
		{workspace}
		{workspaceReadContext}
		{selectedNode}
		sidebarNavigationRevision={controller.sidebarNavigationRevision}
		bind:expandedSections={controller.expandedSections}
		bind:navigationOpen={controller.navigationOpen}
		onNodeSelect={(id) => selectNode(id, false)}
	/>

	<SidebarInset
		class="flex h-full min-w-0 flex-col overflow-hidden xl:rounded-xl xl:border xl:border-sidebar-border xl:bg-background"
	>
		<WorkspaceHeader
			clusterContext={workspace.scope.clusterContext}
			locationLabel={resourceBrowserScope.canQuery ? "Resources" : title}
			focusApplicationName={resourceGitOpsFocusApplication?.name}
			onOpenNavigation={() => (controller.navigationOpen = true)}
			onClusterChange={changeClusterContext}
			onOpenPortForwards={openPortForwards}
			onOpenLauncher={openLauncher}
			onOpenSettings={openSettings}
			onOpenSearch={() => (commandOpen = true)}
		/>

		<YamlDraftNotice />
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
							initialPathState={controller.resourceDetailPathState}
							onPinToggle={() => togglePinnedResource(focusedResource)}
							onClose={() => applyWorkspaceNavigation({ type: "clearResource" })}
							onOpenHelmRelease={openHelmReleaseFromResource}
							onPathStateChange={(state) => (controller.resourceDetailPathState = state)}
						/>
					{/if}
				{/snippet}
			<ScrollArea class="h-full">
				{#if browserMockSurfaces && remoteNoticeShown}
					{#await browserMockSurfaces.remoteNotice() then { default: RemoteWorkspaceNotice }}
						<RemoteWorkspaceNotice />
					{/await}
				{:else if isOverview}
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
							defaultKinds={resourceGitOpsFocusApplication ? GITOPS_RESOURCE_KINDS : workspace.scope.kinds}
							kindScopeLocked={fixedResourceKindScope}
							selectAddedKinds={!resourceBrowserScope.categoryKinds}
							customResourcesEnabled={showCustomResources}
							{customResourcesStatus}
							initialSearch={resourceInitialSearch}
							initialGitOpsFilter={resourceInitialGitOpsFilter}
							initialHealthFilter={resourceInitialHealthFilter}
							gitOpsFocusApplication={resourceGitOpsFocusApplication}
							targetResource={restoreTargetResource ?? focusedResource}
							selectedResource={focusedResource}
							{title}
							initialPathState={controller.navigation.resourceBrowserPathState}
							{pinnedResourceKeys}
							onPathStateChange={(pathState) =>
								applyWorkspaceNavigation({ type: "updateResourceBrowserPath", pathState })}
							onResourceSelect={focusResource}
							canCloseResource={canLeaveYamlDraft}
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
						{helmContext}
						{targetGitOpsApplication}
						{initialIncidentFilter}
						initialPathState={controller.initialSurfacesPathState}
						onOpenResources={openResources}
						onResourceInspect={inspectResource}
						onResourceSelect={selectResource}
						onTargetHelmReleaseResolved={clearTargetHelmRelease}
						onTargetGitOpsApplicationResolved={clearTargetGitOpsApplication}
						onPathStateChange={(state) => (controller.surfacesPathState = state)}
						rbacVerifierHandoff={controller.rbacVerifierHandoff}
						onRbacVerifierHandoffConsumed={() => (controller.rbacVerifierHandoff = undefined)}
						onRbacViewChange={(rbacView) =>
							selectNode({ type: "kind", section: "rbac", kind: rbacView })}
						onRbacVerifierReturn={controller.rbacVerifierReturnNavigation ? returnFromRbacVerifier : undefined}
						rbacVerifierReturnLabel={controller.rbacVerifierReturnLabel}
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
