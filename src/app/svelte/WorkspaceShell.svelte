<script lang="ts">
	import { onMount } from "svelte";
	import { createQuery, useQueryClient } from "@tanstack/svelte-query";
	import {
		Cable,
		FolderOpen,
		Eye,
		Menu,
		Pin,
		Play,
		Search,
		Settings,
		X,
	} from "lucide-svelte";
	import FriendlyError from "@/components/FriendlyError.svelte";
	import {
		Badge,
		Button,
		Alert,
		AlertDescription,
		AlertTitle,
		Card,
		CardContent,
		CardDescription,
		CardHeader,
		CardTitle,
		Checkbox,
		Empty,
		EmptyDescription,
		EmptyHeader,
		EmptyTitle,
		Field,
		FieldLabel,
		ScrollArea,
		Sheet,
		SheetContent,
		SheetTitle,
		Sidebar,
		SidebarContent,
		SidebarInset,
		SidebarProvider,
	} from "@/components/ui/svelte";
	import ClusterSelector from "@/components/ClusterSelector.svelte";
	import {
		summarizeWorkspaceScope,
		workspaceScopeContexts,
		type SavedWorkspace,
	} from "@/lib/workspace-model";
	import type { ArgoApplicationSummary } from "@/lib/gitops-types";
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
	import { createTauriClient, listPresentCustomResourceKinds } from "@/lib/tauri";
	import {
		writePathState,
		type PathStateDetailTab,
		type PathStateResourceDetailState,
		type PathStateSurfacesState,
		type PathStateWorkspaceSnapshot,
	} from "@/lib/path-state";
	import {
		savedPortForwardStartFailureMessage,
		shouldAutoStartSavedPortForwards,
		shouldShowSavedPortForwardRestorePrompt,
		startSavedPortForwards,
	} from "@/features/live-sessions";
	import ResourceDetailPanel from "@/features/resource-detail/ResourceDetailPanel.svelte";
	import NamespaceList from "@/features/resources/NamespaceList.svelte";
	import ResourceBrowser from "@/features/resources/ResourceBrowser.svelte";
	import { resourceSelectionKey, type HealthFilter } from "@/features/resources/helpers";
	import type { IncidentFilter } from "@/features/incidents";
	import WorkspaceOverview from "@/features/workspaces/WorkspaceOverview.svelte";
	import { workspaceStore } from "@/features/workspaces/workspaceStore";
	import AppSurfaces from "./AppSurfaces.svelte";
	import ActiveLiveSessionsButton from "./ActiveLiveSessionsButton.svelte";
	import AppUsageFooter from "./AppUsageFooter.svelte";
	import CommandPalette from "./CommandPalette.svelte";
	import DetailPanelFrame from "./DetailPanelFrame.svelte";
	import SidebarTree from "./SidebarTree.svelte";
	import UpdateStatusButton from "./UpdateStatusButton.svelte";
	import { getSettingsSnapshot, settingsStore } from "@/lib/settings-store";
	import {
		GITOPS_RESOURCE_KINDS,
		appendPresentCustomResourceKinds,
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

	const IS_MAC =
		typeof navigator !== "undefined" && /Mac/i.test(navigator.userAgent);
	const SEARCH_SHORTCUT_HINT = IS_MAC ? "⌘K" : "Ctrl K";

	let {
		workspace,
		initialPathState = null,
		onPathStateConsumed = () => {},
		liveSessionCleanupMessage = null,
		onDismissLiveSessionCleanup = () => {},
	}: {
		workspace: SavedWorkspace;
		initialPathState?: PathStateWorkspaceSnapshot | null;
		onPathStateConsumed?: () => void;
		liveSessionCleanupMessage?: string | null;
		onDismissLiveSessionCleanup?: () => void;
	} = $props();

	const client = createTauriClient();
	const queryClient = useQueryClient();
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
	let navigation = $state<WorkspaceNavigationState>(initialNavigation);
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
	let dismissedPortForwardRestoreWorkspaceId = $state<string | null>(null);
	let startingSavedPortForwards = $state(false);
	let savedPortForwardRestoreError = $state<unknown>(null);
	const autoStartedSavedPortForwardWorkspaceIds = new Set<string>();
	const autoStartSavedPortForwards = $derived(
		$settingsStore.autoStartSavedPortForwards,
	);
	const kubeconfigSourceKey = $derived($settingsStore.kubeconfigSourceKey);
	const showCustomResources = $derived($settingsStore.showCustomResources);

	const navigationModel = $derived(
		buildWorkspaceNavigationModel(workspace, navigation),
	);
	const title = $derived(navigationModel.title);
	const placeholder = $derived(navigationModel.placeholder);
	const scopeSummary = $derived(summarizeWorkspaceScope(workspace.scope));
	const contextCount = $derived(workspaceScopeContexts(workspace.scope).length);
	const isOverview = $derived(navigationModel.isOverview);
	const isNamespaceList = $derived(navigationModel.isNamespaceList);
	const resourceBrowserScope = $derived(navigationModel.resourceBrowserScope);
	const resourceBrowserNamespaces = $derived(resourceBrowserScope.namespaces);
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
		enabled: showCustomResources && Boolean(workspace.scope.clusterContext),
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
		appendPresentCustomResourceKinds(
			resourceBrowserInitialKinds,
			presentCustomResourceKinds,
		),
	);
	const showPortForwardRestorePrompt = $derived(
		shouldShowSavedPortForwardRestorePrompt({
			workspace,
			autoStart: autoStartSavedPortForwards,
			dismissedWorkspaceId: dismissedPortForwardRestoreWorkspaceId,
		}),
	);
	const pinnedResourceKeys = $derived(
		normalizeEntryPoints(workspace.entryPoints).pinned.flatMap((entry) => {
			const resource = resourceFromEntryPoint(entry);
			return resource ? [resourceSelectionKey(resource)] : [];
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
		if (initialPathSnapshot) onPathStateConsumed();
		const closeNarrowNavigation = () => {
			if (window.matchMedia("(min-width: 80rem)").matches) navigationOpen = false;
		};
		window.addEventListener("resize", closeNarrowNavigation);
		return () => window.removeEventListener("resize", closeNarrowNavigation);
	});

	function applyWorkspaceNavigation(intent: WorkspaceNavigationIntent) {
		navigation = navigateWorkspace(navigation, intent);
	}

	$effect(() => {
		if (
			!shouldAutoStartSavedPortForwards({
				workspace,
				autoStart: autoStartSavedPortForwards,
				startedWorkspaceIds: autoStartedSavedPortForwardWorkspaceIds,
			})
		) {
			return;
		}
		autoStartedSavedPortForwardWorkspaceIds.add(workspace.id);
		void startAllSavedPortForwards(false);
	});

	function openLauncher() {
		applyWorkspaceNavigation({ type: "openLauncher" });
		workspaceStore.clearSelectedWorkspace();
	}

	function changeClusterContext(clusterContext: string) {
		if (clusterContext === workspace.scope.clusterContext) return;
		workspaceStore.updateWorkspace(workspace.id, {
			name: workspace.name,
			clusterContext,
			clusterContexts: [clusterContext],
			namespaces: [],
			kinds: workspace.scope.kinds,
			shortcutPreferences: workspace.scope.shortcutPreferences,
		});
		dismissedPortForwardRestoreWorkspaceId = null;
		applyWorkspaceNavigation({ type: "changeCluster" });
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
		writePathState({
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
		if (typeof namespace === "string") {
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

	function setAutoStartSavedPortForwards(autoStart: boolean) {
		getSettingsSnapshot().setAutoStartSavedPortForwards(autoStart);
	}

	function dismissPortForwardRestore() {
		dismissedPortForwardRestoreWorkspaceId = workspace.id;
		savedPortForwardRestoreError = null;
	}

	function reviewPortForwards() {
		openPortForwards();
	}

	async function startAllSavedPortForwards(dismissOnSuccess: boolean) {
		startingSavedPortForwards = true;
		savedPortForwardRestoreError = null;
		try {
			const results = await startSavedPortForwards({
				client,
				workspace,
				kubeconfigSource: kubeconfigSourceKey,
				updateSavedPortForward: workspaceStore.updateSavedPortForward,
				invalidateQueries: (options) => queryClient.invalidateQueries(options),
			});
			const failureMessage = savedPortForwardStartFailureMessage(results);
			if (failureMessage) {
				savedPortForwardRestoreError = failureMessage;
				return;
			}
			if (dismissOnSuccess) dismissPortForwardRestore();
		} catch (error) {
			savedPortForwardRestoreError = error;
		} finally {
			startingSavedPortForwards = false;
		}
	}

	function selectNode(nodeId: TreeNodeId) {
		navigationOpen = false;
		if (nodeId.type === "namespace" && nodeId.namespace) {
			workspaceStore.recordRecentNamespace(
				workspace.id,
				workspace.scope.clusterContext,
				nodeId.namespace,
			);
		}
		applyWorkspaceNavigation({ type: "selectNode", node: nodeId });
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
			logAutoFollow: resourceDetailPathState?.logAutoFollow ?? true,
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

	function toggleSection(id: string) {
		expandedSections = expandedSections.includes(id)
			? expandedSections.filter((item) => item !== id)
			: [...expandedSections, id];
	}

</script>

<SidebarProvider class="h-screen overflow-hidden bg-background text-foreground">
	<Sidebar class="hidden w-[260px] shrink-0 flex-col border-r bg-surface-1 xl:flex">
		<div class="border-b px-3 py-3">
			<div class="flex items-center justify-between gap-2">
				<div class="min-w-0">
					<div class="truncate text-sm font-semibold">{workspace.name}</div>
					<div class="truncate text-xs text-muted-foreground">
						{workspace.scope.clusterContext}
					</div>
				</div>
				<Badge variant="outline" class="tabular-nums">{contextCount}</Badge>
			</div>
		</div>
		<SidebarContent>
			<SidebarTree
				clusterContext={workspace.scope.clusterContext}
				{selectedNode}
				{expandedSections}
				onNodeSelect={selectNode}
				onSectionToggle={toggleSection}
			/>
		</SidebarContent>
	</Sidebar>

	{#if navigationOpen}
		<Sheet open onOpenChange={(open: boolean) => (navigationOpen = open)}>
		<SheetContent side="left" class="w-[280px] max-w-[85vw] p-0 xl:hidden">
			<SheetTitle class="sr-only">Workspace navigation</SheetTitle>
			<div class="border-b px-3 py-3 pr-12">
				<div class="flex items-center justify-between gap-2">
					<div class="min-w-0">
						<div class="truncate text-sm font-semibold">{workspace.name}</div>
						<div class="truncate text-xs text-muted-foreground">
							{workspace.scope.clusterContext}
						</div>
					</div>
					<Badge variant="outline" class="tabular-nums">{contextCount}</Badge>
				</div>
			</div>
			<SidebarContent>
				<SidebarTree
					clusterContext={workspace.scope.clusterContext}
					{selectedNode}
					{expandedSections}
					onNodeSelect={selectNode}
					onSectionToggle={toggleSection}
				/>
			</SidebarContent>
		</SheetContent>
	</Sheet>
	{/if}

	<SidebarInset class="flex h-screen min-w-0 flex-col overflow-hidden">
		<header
			class="flex h-12 shrink-0 items-center gap-2 border-b bg-sidebar px-2 sm:gap-4 sm:px-4 [-webkit-app-region:drag]"
		>
			<Button
				type="button"
				variant="ghost"
				size="icon"
				class="shrink-0 xl:hidden [-webkit-app-region:no-drag]"
				aria-label="Open workspace navigation"
				onclick={() => (navigationOpen = true)}
			>
				<Menu />
			</Button>
			<div class="hidden min-w-0 flex-1 items-center gap-3 [-webkit-app-region:no-drag] md:flex">
				<ClusterSelector
					value={workspace.scope.clusterContext}
					onClusterChange={changeClusterContext}
				/>
				<Badge variant="secondary" class="max-w-48 truncate">{workspace.name}</Badge>
			</div>
			<div class="flex min-w-0 flex-1 items-center justify-center md:flex-none md:basis-1/3">
				<span class="truncate whitespace-nowrap text-sm font-semibold">{title}</span>
			</div>
			<div class="flex min-w-0 shrink-0 items-center justify-end gap-1 [-webkit-app-region:no-drag] md:flex-1">
				<ActiveLiveSessionsButton onOpenManager={openPortForwards} />
				<UpdateStatusButton />
				<Button
					type="button"
					variant="ghost"
					size="icon"
					aria-label="Open workspaces"
					onclick={openLauncher}
				>
					<FolderOpen />
				</Button>
				<Button
					type="button"
					variant="ghost"
					size="icon"
					aria-label="Open settings"
					onclick={openSettings}
				>
					<Settings />
				</Button>
				<Button
					type="button"
					variant="outline"
					size="sm"
					class="hidden gap-2 whitespace-nowrap bg-surface-1 px-3 text-xs font-normal text-muted-foreground shadow-xs [-webkit-app-region:no-drag] hover:bg-surface-2 hover:text-foreground hover:shadow-sm lg:inline-flex"
					aria-label="Search views, namespaces, and resources"
					onclick={() => (commandOpen = true)}
				>
					<Search class="size-3.5" aria-hidden="true" />
					<span>Search resources...</span>
					<kbd class="rounded border bg-muted px-1 py-px font-mono text-xs text-muted-foreground">
						{SEARCH_SHORTCUT_HINT}
					</kbd>
				</Button>
			</div>
		</header>

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

		{#if showPortForwardRestorePrompt}
			<Alert class="shrink-0 rounded-none border-x-0 border-t-0">
				<Cable class="size-3.5" />
				<AlertTitle>Saved port forwards available</AlertTitle>
				<AlertDescription class="flex flex-wrap items-center justify-between gap-3">
					<span>
						This workspace has {workspace.portForwards?.length ?? 0} saved Service{" "}
						{(workspace.portForwards?.length ?? 0) === 1 ? "forward" : "forwards"} ready to start.
					</span>
					{#if savedPortForwardRestoreError}
						<div class="w-full">
							<FriendlyError
								mode="compact"
								error={savedPortForwardRestoreError}
								context={{
									operation: "portForward",
									fallbackTitle: "Saved port-forwards failed",
									partial: true,
								}}
							/>
						</div>
					{/if}
					<span class="flex flex-wrap items-center gap-2">
						<Field orientation="horizontal" class="w-auto pr-2">
							<FieldLabel class="items-center gap-2 text-xs text-muted-foreground">
								<Checkbox
									checked={autoStartSavedPortForwards}
									onCheckedChange={setAutoStartSavedPortForwards}
									aria-label="Auto-start saved port forwards"
								/>
								Auto-start
							</FieldLabel>
						</Field>
						<Button
							type="button"
							size="sm"
							disabled={startingSavedPortForwards}
							onclick={() => void startAllSavedPortForwards(true)}
						>
							<Play data-icon="inline-start" />
							Start saved
						</Button>
						<Button type="button" size="sm" variant="outline" onclick={reviewPortForwards}>
							<Eye data-icon="inline-start" />
							Review
						</Button>
						<Button type="button" size="sm" variant="ghost" onclick={dismissPortForwardRestore}>
							<X data-icon="inline-start" />
							Skip
						</Button>
					</span>
				</AlertDescription>
			</Alert>
		{/if}

		<main class="flex min-h-0 flex-1 overflow-hidden">
			<DetailPanelFrame
				detailOpen={resourceInspectorOpen}
				sizeKey={resourceInspectorSizeKey}
				detailDefaultSize={resourceInspectorDefaultSize}
				detailMinSize={resourceInspectorMinSize}
			>
				{#snippet detailPanel()}
					{#if focusedResource}
						<section class="flex h-full min-h-0 flex-col bg-surface-1">
							<header class="flex shrink-0 items-start justify-between gap-3 border-b px-4 py-3">
								<div class="min-w-0">
									<h2 class="truncate text-sm font-semibold">{focusedResource.name}</h2>
									<div class="truncate text-xs text-muted-foreground">
										{focusedResource.kind} / {focusedResource.namespace ?? "cluster"}
									</div>
								</div>
								<div class="flex shrink-0 items-center gap-1">
									<Button
										type="button"
										variant="ghost"
										size="icon"
										class="size-7 text-muted-foreground"
										aria-label={`${focusedResourcePinned ? "Unpin" : "Pin"} ${focusedResource.kind} ${focusedResource.name}`}
										onclick={() => togglePinnedResource(focusedResource)}
									>
										<Pin class={focusedResourcePinned ? "fill-current" : ""} />
									</Button>
									<Button
										type="button"
										variant="ghost"
										size="icon"
										class="size-7 text-muted-foreground"
										aria-label="Close resource details"
										onclick={() => applyWorkspaceNavigation({ type: "clearResource" })}
									>
										<X />
									</Button>
								</div>
							</header>
							<div class="min-h-0 flex-1 overflow-y-auto overscroll-contain p-4">
								{#key resourceSelectionKey(focusedResource)}
									<ResourceDetailPanel
										{client}
										resource={focusedResource}
										{kubeconfigSourceKey}
										onOpenHelmRelease={openHelmReleaseFromResource}
										initialPathState={resourceDetailPathState}
										onPathStateChange={(state) => (resourceDetailPathState = state)}
									/>
								{/key}
							</div>
						</section>
					{/if}
				{/snippet}
			<ScrollArea class="h-full">
				{#if isOverview}
					<WorkspaceOverview
						{workspace}
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
								<NamespaceList clusterContext={workspace.scope.clusterContext} />
							</CardContent>
						</Card>
					</section>
				{:else if resourceBrowserScope.canQuery}
					<section class="flex h-full min-h-0 w-full flex-col gap-4 p-4 md:p-6">
						<ResourceBrowser
							clusterContext={workspace.scope.clusterContext}
							initialNamespaces={resourceBrowserNamespaces}
							initialKinds={resourceBrowserInitialKinds}
							availableKinds={resourceBrowserKinds}
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
						/>
					</section>
				{:else if viewMode === "argo" || viewMode === "helm" || viewMode === "rbac" || viewMode === "incidents" || viewMode === "portForwards" || viewMode === "settings"}
					<AppSurfaces
						{workspace}
						{viewMode}
						{selectedNode}
						{targetHelmRelease}
						{targetGitOpsApplication}
						selectedResource={focusedResource}
						{initialIncidentFilter}
						initialPathState={initialSurfacesPathState}
					onOpenResources={openResources}
					onResourceInspect={inspectResource}
					onResourceSelect={selectResource}
						onTargetHelmReleaseResolved={clearTargetHelmRelease}
						onTargetGitOpsApplicationResolved={clearTargetGitOpsApplication}
						onPathStateChange={(state) => (surfacesPathState = state)}
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
	onNodeSelect={selectNode}
	onResourceSelect={selectResource}
	onOpenLauncher={openLauncher}
	onOpenSettings={openSettings}
/>
