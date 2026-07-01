<script lang="ts">
	import { onMount } from "svelte";
	import { createQuery, useQueryClient } from "@tanstack/svelte-query";
	import {
		Cable,
		FolderOpen,
		Eye,
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
	import { createTauriClient, listPresentCustomResourceKinds } from "@/lib/tauri";
	import {
		resourceRefFromSummary,
		resourceSummaryFromRef,
		writePathState,
		type PathStateResourceBrowserState,
		type PathStateResourceDetailState,
		type PathStateSurfacesState,
		type PathStateWorkspaceSnapshot,
	} from "@/lib/path-state";
	import {
		savedPortForwardStartFailureMessage,
		shouldAutoStartSavedPortForwards,
		shouldShowSavedPortForwardRestorePrompt,
	} from "@/features/live-sessions/restore";
	import { startSavedPortForwards } from "@/features/live-sessions/saved-port-forward-actions";
	import ResourceDetailPanel from "@/features/resource-detail/ResourceDetailPanel.svelte";
	import NamespaceList from "@/features/resources/NamespaceList.svelte";
	import ResourceBrowser from "@/features/resources/ResourceBrowser.svelte";
	import { resourceSelectionKey, type HealthFilter } from "@/features/resources/helpers";
	import type { IncidentFilter } from "@/features/incidents/helpers";
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
		DEFAULT_WORKSPACE_VIEW,
		GITOPS_RESOURCE_KINDS,
		appendPresentCustomResourceKinds,
		getResourceBrowserScope,
		getWorkspacePlaceholder,
		getWorkspaceTitle,
		isNamespaceListView,
		viewModeForTreeNode,
		type WorkspaceViewMode,
	} from "./workspaceShellModel";

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
	function initialPathSnapshotForWorkspace(): PathStateWorkspaceSnapshot | null {
		return initialPathState?.workspaceId === workspace.id ? initialPathState : null;
	}

	const initialPathSnapshot = initialPathSnapshotForWorkspace();
	const initialResourcePathState = initialPathSnapshot?.resources ?? null;
	const initialDetailPathState = initialPathSnapshot?.detail ?? null;
	const initialSurfacesPathState = initialPathSnapshot?.surfaces ?? null;
	const initialRestoreTargetResource =
		initialPathSnapshot?.restoreTargetResource
			? resourceSummaryFromRef(initialPathSnapshot.restoreTargetResource)
			: null;
	const defaultSelectedNode: TreeNodeId = {
		type: "section",
		section: "workspaceOverview",
	};
	const initialSelectedNode: TreeNodeId | null =
		initialPathSnapshot && "selectedNode" in initialPathSnapshot
				? (initialPathSnapshot.selectedNode ?? null)
			: defaultSelectedNode;
	let selectedNode = $state<TreeNodeId | null>(initialSelectedNode);
	let expandedSections = $state<string[]>(
		initialPathSnapshot?.expandedSections ??
			(initialSelectedNode ? [nodeIdToString(initialSelectedNode)] : []),
	);
	let viewMode = $state<WorkspaceViewMode>(
		initialPathSnapshot?.viewMode ??
			DEFAULT_WORKSPACE_VIEW,
	);
	let commandOpen = $state(false);
	let focusedResource = $state<ResourceSummary | null>(null);
	let restoreTargetResource = $state<ResourceSummary | null>(initialRestoreTargetResource);
	let targetHelmRelease = $state<{ name: string; namespace?: string | null } | null>(
		initialPathSnapshot?.targetHelmRelease ??
			initialSurfacesPathState?.selectedHelmRelease ??
			null,
	);
	let targetGitOpsApplication = $state<string | null>(
		initialPathSnapshot?.targetGitOpsApplication ??
			initialSurfacesPathState?.selectedGitOpsApplication ??
			null,
	);
	let resourceGitOpsFocusApplication = $state<ArgoApplicationSummary | null>(null);
	let resourceInitialSearch = $state(
		initialPathSnapshot?.resourceInitialSearch ??
			"",
	);
	let resourceInitialGitOpsFilter = $state(
		initialPathSnapshot?.resourceInitialGitOpsFilter ??
			"",
	);
	let resourceInitialHealthFilter = $state<HealthFilter>(
		initialPathSnapshot?.resourceInitialHealthFilter ??
			"all",
	);
	let resourceNamespaceOverride = $state<string[] | null>(
		initialPathSnapshot?.resourceNamespaceOverride ??
			null,
	);
	let initialIncidentFilter = $state<IncidentFilter>(
		initialSurfacesPathState?.incidentFilter ?? "all",
	);
	let resourceBrowserPathState = $state<PathStateResourceBrowserState | null>(
		initialResourcePathState,
	);
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

	const title = $derived(getWorkspaceTitle({
		workspace,
		selectedNode,
		viewMode,
	}));
	const placeholder = $derived(getWorkspacePlaceholder({
		selectedNode,
		viewMode,
	}));
	const scopeSummary = $derived(summarizeWorkspaceScope(workspace.scope));
	const contextCount = $derived(workspaceScopeContexts(workspace.scope).length);
	const isOverview = $derived(viewMode === "overview");
	const isNamespaceList = $derived(isNamespaceListView({ selectedNode, viewMode }));
	const resourceBrowserScope = $derived(
		getResourceBrowserScope({ workspace, selectedNode, viewMode }),
	);
	const resourceBrowserNamespaces = $derived(
		resourceNamespaceOverride ?? resourceBrowserScope.namespaces,
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
	const resourceInspectorOpen = $derived(focusedResource !== null);
	const resourceInspectorSizeKey = $derived(viewMode === "argo" ? "gitops" : "resource");
	const resourceInspectorDefaultSize = $derived(viewMode === "argo" ? 30 : 40);
	const resourceInspectorMinSize = $derived(viewMode === "argo" ? 25 : 33);

	onMount(() => {
		if (initialPathSnapshot) onPathStateConsumed();
	});

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
		targetHelmRelease = null;
		targetGitOpsApplication = null;
		restoreTargetResource = null;
		focusedResource = null;
		resourceInitialSearch = "";
		resourceInitialGitOpsFilter = "";
		resourceInitialHealthFilter = "all";
		resourceNamespaceOverride = null;
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
		targetHelmRelease = null;
		targetGitOpsApplication = null;
		restoreTargetResource = null;
		focusedResource = null;
		resourceGitOpsFocusApplication = null;
		resourceInitialSearch = "";
		resourceInitialGitOpsFilter = "";
		resourceInitialHealthFilter = "all";
		resourceNamespaceOverride = null;
		dismissedPortForwardRestoreWorkspaceId = null;
		selectedNode = null;
		viewMode = "resources";
	}

	function currentPathState(): PathStateWorkspaceSnapshot {
		const selectedResourceRef = focusedResource ? resourceRefFromSummary(focusedResource) : null;
		const restoreResourceRef = focusedResource
			? selectedResourceRef
			: restoreTargetResource
				? resourceRefFromSummary(restoreTargetResource)
				: null;
		return {
			workspaceId: workspace.id,
			selectedNode,
			expandedSections,
			viewMode,
			resourceInitialSearch,
			resourceInitialGitOpsFilter,
			resourceInitialHealthFilter,
			resourceNamespaceOverride,
			focusedResource: selectedResourceRef,
			restoreTargetResource: restoreResourceRef,
			targetHelmRelease,
			targetGitOpsApplication,
			resources: resourceBrowserPathState,
			detail: resourceDetailPathState,
			surfaces: surfacesPathState,
		};
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
		targetHelmRelease = null;
		targetGitOpsApplication = null;
		restoreTargetResource = null;
		focusedResource = null;
		resourceInitialSearch = "";
		resourceInitialGitOpsFilter = "";
		resourceInitialHealthFilter = "all";
		resourceNamespaceOverride = null;
		selectedNode = null;
		viewMode = "settings";
	}

	function closeSettings() {
		selectedNode = defaultSelectedNode;
		viewMode = "overview";
	}

	function openResources(
		namespace?: string | string[],
		initialSearch = "",
		initialGitOpsFilter = workspace.scope.gitOpsFilter ?? workspace.scope.argoAppFilter ?? "",
		initialHealthFilter: HealthFilter = "all",
		gitOpsFocusApplication: ArgoApplicationSummary | null = null,
	) {
		targetHelmRelease = null;
		targetGitOpsApplication = null;
		restoreTargetResource = null;
		focusedResource = null;
		resourceGitOpsFocusApplication = gitOpsFocusApplication;
		resourceInitialSearch = initialSearch;
		resourceInitialGitOpsFilter = initialGitOpsFilter;
		resourceInitialHealthFilter = initialHealthFilter;
		resourceBrowserPathState = null;
		resourceNamespaceOverride = Array.isArray(namespace) ? namespace : null;
		selectedNode = typeof namespace === "string"
			? { type: "namespace", section: "namespaces", namespace }
			: null;
		viewMode = "resources";
	}

	function openHelmReleaseFromResource(releaseName: string, namespace?: string | null) {
		targetGitOpsApplication = null;
		restoreTargetResource = null;
		focusedResource = null;
		resourceInitialSearch = "";
		resourceInitialGitOpsFilter = "";
		resourceInitialHealthFilter = "all";
		resourceNamespaceOverride = null;
		targetHelmRelease = { name: releaseName, namespace };
		selectedNode = { type: "section", section: "helm" };
		viewMode = "helm";
	}

	function clearTargetHelmRelease() {
		targetHelmRelease = null;
	}

	function clearTargetGitOpsApplication() {
		targetGitOpsApplication = null;
	}

	function openArgo(argoApp?: string) {
		targetHelmRelease = null;
		targetGitOpsApplication = argoApp ?? null;
		restoreTargetResource = null;
		focusedResource = null;
		resourceInitialSearch = "";
		resourceInitialGitOpsFilter = "";
		resourceInitialHealthFilter = "all";
		resourceNamespaceOverride = null;
		selectedNode = { type: "section", section: "argo" };
		viewMode = "argo";
	}

	function openIncidents(filter: IncidentFilter = "all") {
		targetHelmRelease = null;
		targetGitOpsApplication = null;
		restoreTargetResource = null;
		focusedResource = null;
		resourceInitialSearch = "";
		resourceInitialGitOpsFilter = "";
		resourceInitialHealthFilter = "all";
		resourceNamespaceOverride = null;
		initialIncidentFilter = filter;
		selectedNode = { type: "section", section: "incidents" };
		viewMode = "incidents";
	}

	function openPortForwards() {
		targetHelmRelease = null;
		targetGitOpsApplication = null;
		restoreTargetResource = null;
		focusedResource = null;
		resourceInitialSearch = "";
		resourceInitialGitOpsFilter = "";
		resourceInitialHealthFilter = "all";
		resourceNamespaceOverride = null;
		selectedNode = { type: "section", section: "portForwards" };
		viewMode = "portForwards";
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
			});
			await queryClient.invalidateQueries({ queryKey: queryKeys.portForwards() });
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
		targetHelmRelease = null;
		targetGitOpsApplication = null;
		restoreTargetResource = null;
		focusedResource = null;
		resourceGitOpsFocusApplication = null;
		resourceInitialSearch = "";
		resourceInitialGitOpsFilter = "";
		resourceInitialHealthFilter = "all";
		resourceNamespaceOverride = null;
		initialIncidentFilter =
			nodeId.type === "section" && nodeId.section === "incidents" ? "all" : initialIncidentFilter;
		selectedNode = nodeId;
		viewMode = viewModeForTreeNode(nodeId);
	}

	function selectResource(resource: ResourceSummary, nodeId: TreeNodeId) {
		targetHelmRelease = null;
		targetGitOpsApplication = null;
		restoreTargetResource = null;
		resourceGitOpsFocusApplication = null;
		resourceInitialSearch = "";
		resourceInitialGitOpsFilter = "";
		resourceInitialHealthFilter = "all";
		resourceNamespaceOverride = null;
		focusedResource = resource;
		selectedNode = nodeId;
		viewMode = "resources";
	}

	function inspectResource(resource: ResourceSummary) {
		targetHelmRelease = null;
		targetGitOpsApplication = null;
		restoreTargetResource = null;
		focusedResource = resource;
	}

	function toggleSection(id: string) {
		expandedSections = expandedSections.includes(id)
			? expandedSections.filter((item) => item !== id)
			: [...expandedSections, id];
	}

</script>

<SidebarProvider class="h-screen overflow-hidden bg-background text-foreground">
	<Sidebar class="flex w-[260px] shrink-0 flex-col border-r bg-surface-1">
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

	<SidebarInset class="flex h-screen min-w-0 flex-col overflow-hidden">
		<header
			class="flex h-12 shrink-0 items-center gap-4 border-b bg-sidebar px-4 [-webkit-app-region:drag]"
		>
			<div class="flex min-w-0 flex-1 items-center gap-3 [-webkit-app-region:no-drag]">
				<ClusterSelector
					value={workspace.scope.clusterContext}
					onClusterChange={changeClusterContext}
				/>
				<Badge variant="secondary" class="max-w-48 truncate">{workspace.name}</Badge>
			</div>
			<div class="flex min-w-0 flex-1 items-center justify-center">
				<span class="truncate whitespace-nowrap text-sm font-semibold">{title}</span>
			</div>
			<div class="flex flex-1 items-center justify-end gap-1 [-webkit-app-region:no-drag]">
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
					class="gap-2 whitespace-nowrap bg-surface-1 px-3 text-xs font-normal text-muted-foreground shadow-xs [-webkit-app-region:no-drag] hover:bg-surface-2 hover:text-foreground hover:shadow-sm"
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
								<Button
									type="button"
									variant="ghost"
									size="icon"
									class="size-7 text-muted-foreground"
									aria-label="Close resource details"
									onclick={() => {
										focusedResource = null;
										restoreTargetResource = null;
									}}
								>
									<X />
								</Button>
							</header>
							<div class="min-h-0 flex-1 overflow-y-auto overscroll-contain p-4">
								{#key resourceSelectionKey(focusedResource)}
									<ResourceDetailPanel
										{client}
										resource={focusedResource}
										{kubeconfigSourceKey}
										onOpenHelmRelease={openHelmReleaseFromResource}
										initialPathState={initialDetailPathState}
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
							initialPathState={resourceBrowserPathState}
							onPathStateChange={(state) => (resourceBrowserPathState = state)}
							onResourceSelect={(resource) => {
								focusedResource = resource;
								restoreTargetResource = null;
							}}
							onResourceClose={() => {
								focusedResource = null;
								restoreTargetResource = null;
							}}
						/>
					</section>
				{:else if viewMode === "argo" || viewMode === "helm" || viewMode === "rbac" || viewMode === "incidents" || viewMode === "portForwards" || viewMode === "settings"}
					<AppSurfaces
						{workspace}
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
