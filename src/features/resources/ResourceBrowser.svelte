<script lang="ts">
	import ResourceScopeWarnings from "./ResourceScopeWarnings.svelte";
	import ResourceLiveStatusMenu from "@/components/ResourceLiveStatusMenu.svelte";
	import { markStartup } from "@/lib/startup-marks";
	import FriendlyError from "@/components/FriendlyError.svelte";
	import { Empty, EmptyDescription, EmptyHeader, EmptyTitle, Spinner } from "@/components/ui/svelte";
	import ResourceBrowserMapPane from "./ResourceBrowserMapPane.svelte";
	import ResourceTablePane from "./ResourceTablePane.svelte";
	import { argoApplicationInspection } from "@/lib/argo-application-inspection.svelte";
	import { loadingDelay } from "@/lib/loading-delay.svelte";
	import ArgoApplicationWorkspaceHeader from "@/features/gitops/ArgoApplicationWorkspaceHeader.svelte";
	import {
		argoResourceCounts,
		argoResourceIdentityKey,
		filterWorkspaceResourcesByArgo,
		type ArgoResourceFilter,
	} from "@/features/gitops/argo-workspace-model";
	import type { ArgoApplicationSummary } from "@/lib/gitops-types";
	import { settingsStore } from "@/lib/settings-store";
	import { resourceKindOrder } from "@/lib/resource-kind-order";
	import type { PathStateResourceBrowserState } from "@/lib/path-state";
	import { createTauriClient } from "@/lib/tauri";
	import type { ResourceKindSelection, ResourceSummary } from "@/lib/types";
	import type { WorkspaceReadContext } from "@/lib/workspaceReadContext";
	import { findResourceIndex, resourceKey } from "@/lib/resource-identity";
	import { PAGE_SIZE } from "./constants";
	import { kindFetchKeys, kindKey } from "@/lib/kind-identity";
	import { contextsLoadingNotice } from "./contextResults";
	import type { HealthFilter } from "./helpers";
	import ResourceBrowserTopBar from "./ResourceBrowserTopBar.svelte";
	import {
		allKindOptions,
		filterResourcesByKinds,
		createTableTopologyFilter,
		nextNamespaceSelection,
		syncedTopologyNodeId as resolveSyncedTopologyNodeId,
		topologyLinkKinds,
	} from "./resourceBrowserModel";
	import { resourceBrowserScope } from "./resourceBrowserScope.svelte";
	import { resourceBrowserReads } from "./resourceBrowserReads.svelte";
	import {
		buildResourceTableDisplayFromMembership,
		buildResourceTableMembership,
		sortResourceTableRows,
		buildResourceTablePage,
		buildResourceTableProjection,
	} from "./resourceTableModel";
	import { createResourceTableEntryBuilder } from "./resourceTableEntries";

	let {
		client: providedClient,
		workspaceReadContext,
		initialNamespaces,
		initialKinds,
		availableKinds = initialKinds,
		defaultKinds = initialKinds,
		kindScopeLocked = false,
		selectAddedKinds = true,
		customResourcesEnabled = true,
		customResourcesStatus = null,
		initialSearch = "",
		initialGitOpsFilter = "",
		initialHealthFilter = "all",
		gitOpsFocusApplication = null,
		targetResource,
		selectedResource = null,
		title,
		initialPathState = null,
		pinnedResourceKeys = [],
		onResourceSelect = () => {},
		onResourcePinToggle = () => {},
		onResourceClose = () => {},
		canCloseResource = () => true,
		onArgoApplicationInspect = () => {},
		onPathStateChange = () => {},
	}: {
		client?: import("@/lib/tauri").TauriClient;
		workspaceReadContext: WorkspaceReadContext;
		initialNamespaces: string[];
		initialKinds: ResourceKindSelection[];
		availableKinds?: ResourceKindSelection[];
		/** The workspace's default kinds. A selection that leaves one out also maps resources one edge from the selected kinds. */
		defaultKinds?: ResourceKindSelection[];
		kindScopeLocked?: boolean;
		/** Whether available kinds beyond the initial ones, such as discovered custom kinds, join the selection. */
		selectAddedKinds?: boolean;
		customResourcesEnabled?: boolean;
		customResourcesStatus?: string | null;
		initialSearch?: string;
		initialGitOpsFilter?: string;
		initialHealthFilter?: HealthFilter;
		gitOpsFocusApplication?: ArgoApplicationSummary | null;
		targetResource?: ResourceSummary | null;
		selectedResource?: ResourceSummary | null;
		title: string;
		initialPathState?: PathStateResourceBrowserState | null;
		pinnedResourceKeys?: string[];
		onResourceSelect?: (resource: ResourceSummary, source?: "explicit" | "restore") => void;
		onResourcePinToggle?: (resource: ResourceSummary) => void;
		onResourceClose?: () => void;
		/** False keeps the selection and the scope, such as while the open YAML draft has unsaved changes. */
		canCloseResource?: () => boolean;
		onArgoApplicationInspect?: (app: ArgoApplicationSummary) => void;
		onPathStateChange?: (state: PathStateResourceBrowserState) => void;
	} = $props();
	// svelte-ignore state_referenced_locally -- The mounted browser owns one fixed transport.
	const client = providedClient ?? createTauriClient();

	const scope = resourceBrowserScope({
		get initialNamespaces() { return initialNamespaces; },
		get initialKinds() { return initialKinds; },
		get availableKinds() { return availableKinds; },
		get initialSearch() { return initialSearch; },
		get initialGitOpsFilter() { return initialGitOpsFilter; },
		get initialHealthFilter() { return initialHealthFilter; },
		get gitOpsFocusApplication() { return gitOpsFocusApplication; },
		get kindScopeLocked() { return kindScopeLocked; },
		get initialPathState() { return initialPathState; },
		get selectAddedKinds() { return selectAddedKinds; },
		get customResourcesEnabled() { return customResourcesEnabled; },
	}, () => resourcesQuery.isSuccess && !resourcesQuery.isPlaceholderData,
		(state) => onPathStateChange(state), () => tableModel.safePageIndex);
	let hideHistoricalReplicaSets = $state(false);
	let appliedTargetResourceKey = $state("");
	const showFullTopologyOnSelection = $derived($settingsStore.showFullTopologyOnSelection);

	const clusterContext = $derived(workspaceReadContext.clusterContext);
	const sourceReady = $derived(workspaceReadContext.sourceReady);
	const fetchKeys = $derived(kindFetchKeys(scope.selectedNamespaces, scope.selectedKinds));
	const focusedArgo = argoApplicationInspection(client, () => ({
		readContext: workspaceReadContext,
		application: gitOpsFocusApplication,
	}));
	const reads = resourceBrowserReads(client, () => ({
		readContext: workspaceReadContext, fetchKeys, namespaces: scope.selectedNamespaces, topologyMode: scope.topologyMode, mapPanelOpen: scope.mapPanelOpen,
		tablePanelOpen: scope.tablePanelOpen, customResourcesEnabled, focusedApplication: gitOpsFocusApplication,
	}));
	const live = reads.live;
	// The Context column, filter and search show only when the workspace has more than one context.
	const multiContext = $derived(reads.contexts.length > 1);
	$effect(() => {
		if (sourceReady && reads.namespacesReady && reads.kindsReady) markStartup("base-scope-ready");
	});
	const resourcesQuery = $derived(reads.scope);
	const topologyQuery = reads.topology;
	const updatesCurrent = $derived(reads.freshness.tableCurrent);
	const mapUpdatesCurrent = $derived(reads.freshness.mapCurrent);
	const notice = $derived(reads.freshness.notice);

	const namespaceOptions = $derived(reads.namespaces);
	const kindOptions = $derived(
		kindScopeLocked
			? [...initialKinds]
			: allKindOptions(customResourcesEnabled ? reads.kinds : []),
	);
	const selectedNamespaceSet = $derived(new Set(scope.selectedNamespaces));
	const selectedKindSet = $derived(new Set(scope.selectedKinds.map(kindKey)));
	const scopedRows = $derived(
		filterResourcesByKinds(resourcesQuery.data?.resources ?? [], scope.selectedKinds),
	);
	const focusedArgoResources = $derived(focusedArgo.current.data?.resources ?? []);
	const focusedArgoFilterSummary = $derived(
		gitOpsFocusApplication && focusedArgo.current.data
			? argoResourceCounts(focusedArgoResources)
			: null,
	);
	const focusedArgoResourceKeys = $derived(
		new Set(
			focusedArgoResources
				.map(argoResourceIdentityKey)
				.filter((key): key is string => key !== null),
		),
	);
	const tableRows = $derived(
		gitOpsFocusApplication
			? filterWorkspaceResourcesByArgo(
					scopedRows,
					focusedArgoResources,
					scope.argoResourceFilter,
				)
			: scopedRows,
	);
	let firstResourceRowsMarked = false;
	$effect(() => {
		if (firstResourceRowsMarked) return;
		if (scope.tablePanelOpen && resourcesQuery.isSuccess && !resourcesQuery.isPlaceholderData && tableModel.entries.some((entry) => entry.type === "resource")) {
			markStartup("first-resource-rows");
			firstResourceRowsMarked = true;
		}
	});
	const tableProjection = $derived(buildResourceTableProjection(tableRows, multiContext));
	const membership = $derived(buildResourceTableMembership(tableProjection, {
		search: scope.filterSearch, gitOpsFilter: scope.gitOpsFilter, healthFilter: scope.healthFilter, contextFilter: new Set(scope.contextFilter),
	}));
	const sortedRows = $derived(sortResourceTableRows(
		membership.filteredRows, { id: scope.sortColumn, desc: scope.sortDesc }, reads.contexts,
		scope.sortColumn === "cpu" || scope.sortColumn === "memory" ? reads.metricsIndex : undefined,
	));
	const tableDisplay = $derived(buildResourceTableDisplayFromMembership(membership, sortedRows, {
		preferredGitOpsResourceKeys: gitOpsFocusApplication ? focusedArgoResourceKeys : undefined,
		kindOrder: resourceKindOrder($settingsStore.resourceKindLanes),
	}));
	const tablePage = $derived(buildResourceTablePage(tableDisplay, scope.pageIndex));
	const buildPageEntries = createResourceTableEntryBuilder();
	const tableModel = $derived({
		...tablePage,
		entries: buildPageEntries(tablePage, scope.collapsedGroups, selectedResource),
	});
	const filterTableTopology = createTableTopologyFilter();
	const topology = $derived(filterTableTopology(
		topologyQuery.data, membership.filteredRows, hideHistoricalReplicaSets,
		topologyLinkKinds(scope.selectedKinds, defaultKinds),
	));
	const inspectorOpen = $derived(Boolean(selectedResource));
	const syncedTopologyNodeId = $derived(
		resolveSyncedTopologyNodeId({
			selectedTopologyNodeId: scope.selectedTopologyNodeId,
			selectedResource,
			topologyNodes: topology?.nodes,
		}),
	);
	const targetResourceKey = $derived(
		targetResource ? resourceKey(targetResource) : "",
	);
	const loading = loadingDelay(
		() =>
			Boolean(clusterContext) &&
			fetchKeys.length > 0 &&
			(!sourceReady || (resourcesQuery.isPending && !resourcesQuery.isPlaceholderData)),
	);
	const contextsLoading = loadingDelay(() => resourcesQuery.loadingContexts.length > 0);
	const contextsLoadingText = $derived(contextsLoadingNotice(resourcesQuery.loadingContexts));
	const staleNoticeDelayMs = 500;
	const tableStalePending = $derived(notice.table !== null && !notice.table.current && !notice.table.error
		&& notice.table.status !== "partial" && notice.table.status !== "polling");
	const mapStalePending = $derived(notice.map !== null && !notice.map.current && !notice.map.failed && !notice.map.error
		&& (!notice.map.sharesTableError || notice.map.status === "reconnecting" || notice.map.status === "connecting")
		&& notice.map.status !== "partial" && notice.map.status !== "polling");
	const tableStale = loadingDelay(() => tableStalePending, staleNoticeDelayMs);
	const mapStale = loadingDelay(() => mapStalePending, staleNoticeDelayMs);
	// Narrow the wording as soon as the panel that is still stale can keep the banner visible.
	const onlyTableStale = $derived(tableStalePending && tableStale.shown && !mapStalePending);
	const onlyMapStale = $derived(mapStalePending && mapStale.shown && !tableStalePending);
	$effect(() => {
		if (onlyTableStale) mapStale.releaseHold();
		if (onlyMapStale) tableStale.releaseHold();
	});
	type NoticeKind = "refreshFailed" | "topologyFailed" | "partial" | "polling" | "stale";
	const tableNotice = $derived(notice.table && (notice.table.error
		? "refreshFailed"
		: watchNotice(notice.table.status, tableStale.shown && !onlyMapStale)));
	const mapWatchNotice = $derived(notice.map && watchNotice(notice.map.status, mapStale.shown && !onlyTableStale));
	const mapNotice = $derived.by((): NoticeKind | null => {
		if (!notice.map) return null;
		if (notice.map.failed) return "topologyFailed";
		if (notice.map.error) return "refreshFailed";
		return mapWatchNotice === tableNotice ? null : mapWatchNotice;
	});
	const sharedNotice = $derived(notice.map?.sharesTableError || (!mapNotice && tableNotice === mapWatchNotice));
	let mapNoticeElement = $state<HTMLDivElement>();
	let mapPanelElement = $state<HTMLDivElement>();
	let tableNoticeElement = $state<HTMLDivElement>();
	let tableHeadingElement = $state<HTMLHeadingElement>();
	let browserElement = $state<HTMLDivElement>();

	function watchNotice(status: string, stale: boolean): NoticeKind | null {
		if (status === "partial" || status === "polling") return status;
		return stale ? "stale" : null;
	}

	function noticeText(kind: NoticeKind, panel?: "Table" | "Map") {
		const activity = panel ? `${panel} activity indicators` : "Activity indicators";
		switch (kind) {
			case "refreshFailed": return `Refresh failed. Showing last observed state. ${activity} are paused until live updates recover.`;
			case "topologyFailed": return "Showing last observed graph. Map activity indicators are paused until the map refreshes.";
			case "partial": return `Live updates are limited. Readable resources without watch access refresh periodically. ${activity} are paused.`;
			case "polling": return `Live updates are unavailable. Readable resources refresh periodically. ${activity} are paused.`;
			case "stale": return `Showing last observed state. ${activity} are paused until live updates recover.`;
		}
	}
	const resourceError = $derived(
		workspaceReadContext.sourceError
			? workspaceReadContext.sourceError
			: resourcesQuery.isError
				? resourcesQuery.error
				: null,
	);
	const errorReplacesBrowser = $derived(Boolean(resourceError && (!resourcesQuery.data || workspaceReadContext.sourceError)));
	// With every kind unchecked the top bar stays, so the kind picker can bring one back without a scope reset.
	const topBarShown = $derived(Boolean(clusterContext) && (fetchKeys.length === 0 || (!errorReplacesBrowser && !loading.active)));
	const panelsShown = $derived(Boolean(clusterContext) && fetchKeys.length > 0 && !errorReplacesBrowser && !loading.active);

	$effect.pre(() => {
		const focused = document.activeElement;
		const mapMounted = panelsShown && scope.mapPanelOpen;
		const tableMounted = panelsShown && scope.tablePanelOpen;
		const hasMapDetails = mapMounted && mapNotice === "refreshFailed";
		if (!hasMapDetails && mapNoticeElement?.contains(focused)) {
			(mapMounted ? mapPanelElement : browserElement)?.focus({ preventScroll: true });
		}
		if (!(tableMounted && tableNotice && notice.table?.error) && tableNoticeElement?.contains(focused)) {
			(tableMounted ? tableHeadingElement : browserElement)?.focus({ preventScroll: true });
		}
	});

	$effect(() => {
		if (!targetResourceKey) {
			appliedTargetResourceKey = "";
			return;
		}
		if (
			appliedTargetResourceKey === targetResourceKey ||
			!resourcesQuery.isSuccess ||
			resourcesQuery.isPlaceholderData
		) return;
		if (!targetResource) return;
		const rowIndex = findResourceIndex(tableModel.displayRows, targetResource, (row) => row);
		if (rowIndex < 0) return;
		const matchedResource = tableModel.displayRows[rowIndex];
		onResourceSelect(matchedResource, "restore");
		scope.selectedTopologyNodeId = null;
		scope.pageIndex = Math.max(0, Math.floor(rowIndex / PAGE_SIZE));
		appliedTargetResourceKey = targetResourceKey;
	});

	function toggleNamespace(namespace: string, checked: boolean) {
		if (!canCloseResource()) return;
		scope.selectedNamespaces = nextNamespaceSelection(
			scope.selectedNamespaces,
			namespaceOptions.map((option) => option.name),
			namespace,
			checked,
		);
		scope.pageIndex = 0;
		onResourceClose();
		scope.selectedTopologyNodeId = null;
	}

	function toggleKind(kind: ResourceKindSelection, checked: boolean) {
		if (!canCloseResource()) return;
		const key = kindKey(kind);
		scope.selectedKinds = checked
			? [...scope.selectedKinds, kind]
			: scope.selectedKinds.filter((item) => kindKey(item) !== key);
		scope.pageIndex = 0;
		onResourceClose();
		scope.selectedTopologyNodeId = null;
	}

	function selectHealth(filter: HealthFilter) {
		scope.healthFilter = filter;
		scope.pageIndex = 0;
	}

	function selectArgoResource(filter: ArgoResourceFilter) {
		scope.argoResourceFilter = scope.argoResourceFilter === filter ? "none" : filter;
		scope.pageIndex = 0;
	}

	function clearFilters() {
		scope.search = "";
		scope.contextFilter = [];
		scope.gitOpsFilter = "";
		scope.healthFilter = "all";
		scope.argoResourceFilter = "none";
		scope.pageIndex = 0;
	}

	function setGitOpsFilter(value: string) {
		scope.gitOpsFilter = value === "__all" ? "" : value;
		scope.pageIndex = 0;
	}

	function selectResource(resource: ResourceSummary) {
		scope.selectedTopologyNodeId = null;
		onResourceSelect(resource, "explicit");
	}

	function selectTopologyResource(nodeId: string, resource: ResourceSummary | null) {
		scope.selectedTopologyNodeId = nodeId;
		if (resource) onResourceSelect(resource, "explicit");
	}

	function closeMapPanel() {
		scope.mapPanelOpen = false;
		scope.selectedTopologyNodeId = null;
	}
</script>

{#snippet refreshControls()}
	{#key live.key}
		<ResourceLiveStatusMenu {live} disabled={!sourceReady || !clusterContext} />
	{/key}
{/snippet}

{#snippet observationNotice(label: string, error: unknown, context?: string)}
	{#if error}
		<details class="rounded-md border border-amber-500/30 text-xs text-amber-700 dark:text-amber-300" data-refresh-error>
			<summary class="cursor-pointer px-3 py-2"><span role="status">{label}</span></summary>
			<div class="px-3 pb-3">
				{#if context}<p class="pb-2">Context {context}</p>{/if}
				<FriendlyError {error} context={{ operation: "resourcesLoad", fallbackTitle: "Resource refresh failed. Retrying." }} />
			</div>
		</details>
	{:else}
		<p role="status" class="rounded-md border border-amber-500/30 px-3 py-2 text-xs text-amber-700 dark:text-amber-300">{label}</p>
	{/if}
{/snippet}

<div class="flex h-full min-h-0 min-w-0 flex-col gap-3" bind:this={browserElement} tabindex="-1">
	{#if resourcesQuery.data && !resourcesQuery.isPlaceholderData}
		<ResourceScopeWarnings failures={resourcesQuery.data?.failures ?? []} contextFailures={resourcesQuery.contextFailures} contextRefreshFailed={resourcesQuery.isError} />
	{/if}
	{#if contextsLoading.shown && resourcesQuery.loadingContexts.length > 0}
		<p role="status" class="flex items-center gap-2 text-xs text-muted-foreground" data-contexts-loading>
			<Spinner class="size-3.5" />
			<span>{contextsLoadingText}</span>
		</p>
	{/if}
	{#if !clusterContext || (fetchKeys.length > 0 && (resourceError || loading.active))}
		<div class="flex justify-end">{@render refreshControls()}</div>
	{/if}
	{#if gitOpsFocusApplication}
		<ArgoApplicationWorkspaceHeader
			app={gitOpsFocusApplication}
			inspector={focusedArgo.current.data ?? null}
			managedResources={focusedArgoResources}
			loading={focusedArgo.current.loading}
			refreshing={focusedArgo.current.refreshing}
			refreshDisabled={!focusedArgo.current.ready}
			error={focusedArgo.current.error}
			onRefresh={focusedArgo.refresh}
			onInspect={onArgoApplicationInspect}
		/>
	{/if}

	{#if topBarShown}
		<ResourceBrowserTopBar
			{kindScopeLocked}
			selectedNamespaces={scope.selectedNamespaces} selectedKinds={scope.selectedKinds}
			namespaceOptions={namespaceOptions}
			kindOptions={kindOptions}
			selectedNamespaceSet={selectedNamespaceSet}
			selectedKindSet={selectedKindSet}
			healthSummary={tableModel.healthSummary}
			healthFilter={scope.healthFilter}
			bind:search={scope.search}
			gitOpsFilter={scope.gitOpsFilter}
			gitOpsFilters={tableModel.gitOpsFilters}
			argoSummary={focusedArgoFilterSummary}
			argoFilter={scope.argoResourceFilter}
			{hideHistoricalReplicaSets}
			metricsCard={reads.metricsCard}
			contexts={multiContext ? reads.contexts : []}
			contextFilter={scope.contextFilter}
			onContextFilterChange={(next) => { scope.contextFilter = next; scope.pageIndex = 0; }}
			{customResourcesStatus}
			{refreshControls}
			onAllNamespacesSelect={() => {
				if (!canCloseResource()) return;
				scope.selectedNamespaces = [];
				scope.pageIndex = 0;
				onResourceClose();
				scope.selectedTopologyNodeId = null;
			}}
			onAllKindsSelect={() => {
				if (!canCloseResource()) return;
				scope.selectedKinds = [...kindOptions];
				scope.pageIndex = 0;
				onResourceClose();
				scope.selectedTopologyNodeId = null;
			}}
			onNamespaceToggle={toggleNamespace}
			onKindToggle={toggleKind}
			onHealthSelect={selectHealth}
			onArgoFilterSelect={selectArgoResource}
			onHideHistoricalReplicaSetsChange={(hide) => (hideHistoricalReplicaSets = hide)}
			onGitOpsFilterChange={setGitOpsFilter}
			onClearFilters={clearFilters}
		/>
	{/if}
	{#if !clusterContext}
		<Empty class="min-h-52 border border-dashed">
			<EmptyHeader>
				<EmptyTitle>No cluster context</EmptyTitle>
				<EmptyDescription>Select a workspace with a cluster context.</EmptyDescription>
			</EmptyHeader>
		</Empty>
	{:else if fetchKeys.length === 0}
		<Empty class="min-h-52 border border-dashed">
			<EmptyHeader>
				<EmptyTitle>No resource scope</EmptyTitle>
				<EmptyDescription>Select at least one kind.</EmptyDescription>
			</EmptyHeader>
		</Empty>
	{:else if resourceError && errorReplacesBrowser}
		<FriendlyError
			error={resourceError}
			context={{ operation: "resourcesLoad", fallbackTitle: "Failed to load resources" }}
		/>
	{:else if loading.active}
		<div class="flex min-h-52 items-center justify-center gap-2 text-xs text-muted-foreground">
			{#if loading.shown}
				<Spinner />
				<span>Loading resources</span>
			{/if}
		</div>
	{:else}
		{#if tableNotice}
			{@const label = noticeText(tableNotice, notice.map && !sharedNotice ? "Table" : undefined)}
			<div class="contents" bind:this={tableNoticeElement}>
				{@render observationNotice(label, notice.table?.error, notice.table?.context)}
			</div>
		{/if}
		{#if mapNotice}
			<div class="contents" bind:this={mapNoticeElement}>
				{@render observationNotice(noticeText(mapNotice, notice.table || notice.map?.error ? "Map" : undefined), notice.map?.error)}
			</div>
		{/if}
		<div
			class={inspectorOpen && scope.mapPanelOpen && scope.tablePanelOpen
				? "grid min-h-0 min-w-0 flex-1 grid-cols-1 grid-rows-[minmax(400px,1fr)_minmax(400px,1fr)] gap-3"
				: scope.mapPanelOpen && scope.tablePanelOpen
					? "grid min-h-0 min-w-0 flex-1 gap-3 min-[1101px]:grid-cols-[minmax(0,0.42fr)_minmax(0,0.58fr)]"
				: scope.mapPanelOpen
					? "grid min-h-0 min-w-0 flex-1 grid-cols-[minmax(0,1fr)_3rem] gap-3"
					: scope.tablePanelOpen
						? "grid min-h-0 min-w-0 flex-1 grid-cols-[3rem_minmax(0,1fr)] gap-3"
						: "grid min-h-0 min-w-0 flex-1 grid-cols-[3rem_3rem] gap-3"}
		>
			<ResourceBrowserMapPane
				bind:mapPanelOpen={scope.mapPanelOpen} bind:mapPanelElement {topology}
				metricsIndex={reads.metricsIndex} updatesCurrent={mapUpdatesCurrent}
				isLoading={topologyQuery.isPending && !topologyQuery.data}
				isError={topologyQuery.isError && !topologyQuery.data} error={topologyQuery.error}
				mode={scope.topologyMode} selectedNodeId={syncedTopologyNodeId} {showFullTopologyOnSelection}
				fitViewKey={reads.topologyFitViewKey} contexts={reads.contexts} mapContext={reads.mapContext}
				onMapContextChange={(context) => { reads.selectMapContext(context); scope.selectedTopologyNodeId = null; }}
				onModeChange={(mode) => { scope.topologyMode = mode; scope.selectedTopologyNodeId = null; }}
				onNodeSelect={selectTopologyResource} onMapToggle={closeMapPanel}
			/>
			<ResourceTablePane
				{tableModel} bind:tablePanelOpen={scope.tablePanelOpen} bind:tableHeadingElement bind:pageIndex={scope.pageIndex}
				bind:sortColumn={scope.sortColumn} bind:sortDesc={scope.sortDesc} bind:collapsedGroups={scope.collapsedGroups}
				{selectedResource} metricsIndex={reads.metricsIndex} contexts={reads.contexts} {title}
				{pinnedResourceKeys} {updatesCurrent} {selectResource} {onResourcePinToggle}
			/>
		</div>
	{/if}
</div>
