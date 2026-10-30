<script module lang="ts">
	let ownershipMapImport: Promise<typeof import("./OwnershipMap.svelte")> | undefined;

	function loadOwnershipMap() {
		return (ownershipMapImport ??= import("./OwnershipMap.svelte"));
	}

	function retryOwnershipMapLoad() {
		ownershipMapImport = undefined;
		return loadOwnershipMap();
	}
</script>

<script lang="ts">
	import ResourceScopeWarnings from "./ResourceScopeWarnings.svelte";
	import type { ResourceScopeResult } from "@/lib/types";
	import ResourceLiveStatusMenu from "@/components/ResourceLiveStatusMenu.svelte";
	import { liveViewRefresh } from "@/lib/live-view-refresh.svelte";
	import { markStartup } from "@/lib/startup-marks";
	import { createQuery } from "@tanstack/svelte-query";
	import {
		ArrowDown,
		ArrowUp,
		ChevronDown,
		ChevronRight,
		ChevronsUpDown,
		GitBranch,
		PanelRightClose,
		PanelRightOpen,
		Pin,
		Table2,
	} from "lucide-svelte";
	import FriendlyError from "@/components/FriendlyError.svelte";
	import ResourceTableStatus from "./ResourceTableStatus.svelte";
	import CopyableText from "@/components/CopyableText.svelte";
	import TimestampText from "@/components/TimestampText.svelte";
	import {
		Badge,
		Button,
		Empty,
		EmptyDescription,
		EmptyHeader,
		EmptyTitle,
		Spinner,
		Table,
		TableBody,
		TableCell,
		TableHead,
		TableHeader,
		TableRow,
	} from "@/components/ui/svelte";
	import {
		getResourceGroupVisual,
		getResourceKindVisual,
	} from "@/app/svelte/resourceVisuals";
	import {
		CPU_USAGE_DESCRIPTION,
		MEMORY_USAGE_DESCRIPTION,
		READINESS_NOT_REPORTED,
		RESTART_COUNT_NOT_REPORTED,
	} from "./operational-data";
	import { argoApplicationInspection } from "@/lib/argo-application-inspection.svelte";
	import { diagnosticLog } from "@/lib/diagnostics";
	import { STATUS_BADGE_STYLES } from "@/components/status-badge-styles";
	import type { ChipVariant } from "@/features/argo/status";
	import ArgoApplicationWorkspaceHeader from "@/features/gitops/ArgoApplicationWorkspaceHeader.svelte";
	import {
		argoResourceCounts,
		argoResourceIdentityKey,
		filterWorkspaceResourcesByArgo,
		type ArgoResourceFilter,
	} from "@/features/gitops/argo-workspace-model";
	import {
		describeMetricsAvailability,
		formatCpuMillicores,
		formatMemoryBytes,
		mergeResourceMetrics,
		mergeTopologyMetrics,
		resourceMetricIndex,
	} from "@/lib/resource-metrics";
	import { gitOpsOwnership } from "@/lib/gitops-ownership-evidence";
		import { cnfast } from "@/lib/utils";
	import type { ArgoApplicationSummary } from "@/lib/gitops-types";
	import { getSettingsSnapshot, settingsStore } from "@/lib/settings-store";
	import { resourceKindOrder } from "@/lib/resource-kind-order";
	import type { PathStateResourceBrowserState } from "@/lib/path-state";
	import { createTauriClient, listNamespaces, listResourceKinds } from "@/lib/tauri";
	import type {
		DiscoveredResourceKind,
		NamespaceSummary,
		ResourceKindSelection,
		ResourceMetricsSummary,
		ResourceSummary,
		ResourceTopology,
		TopologyMode,
	} from "@/lib/types";
	import type { WorkspaceReadContext } from "@/lib/workspaceReadContext";
	import { findResourceIndex, resourceKey } from "@/lib/resource-identity";
	import {
		EMPTY_PAGE_CLASS,
		PAGE_SIZE,
		ROW_CLASS,
		SELECTED_ROW_CLASS,
		STICKY_APP_GROUP_TOP,
		STICKY_TYPE_GROUP_TOP,
		TABLE_CLASS,
	} from "./constants";
	import { isCustomResourceKind, kindFetchKeys, kindKey, sameKinds } from "@/lib/kind-identity";
	import { resourceReadyChip } from "./helpers";
	import type { HealthFilter } from "./helpers";
	import ResourceBrowserTopBar from "./ResourceBrowserTopBar.svelte";
	import {
		allKindOptions,
		filterResourcesByKinds,
		createTableTopologyFilter,
		initialOwnershipMapOpen,
		newlyAvailableKinds,
		nextNamespaceSelection,
		shouldLoadOwnershipMap,
		syncedTopologyNodeId as resolveSyncedTopologyNodeId,
	} from "./resourceBrowserModel";
	import { buildResourceBrowserReadSpecs, resourceBrowserLiveView } from "./resourceBrowserReadSpecs";
	import {
		buildResourceTableDisplay,
		buildResourceTablePage,
		buildResourceTableProjection,
		type ResourceSortColumn,
	} from "./resourceTableModel";
	import { createResourceTableEntryBuilder } from "./resourceTableEntries";

	const BACKGROUND_METRICS_DELAY_MS = 1_500;
	const EMPTY_CELL = "—";

	let {
		client: providedClient,
		workspaceReadContext,
		initialNamespaces,
		initialKinds,
		availableKinds = initialKinds,
		kindScopeLocked = false,
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
		onArgoApplicationInspect = () => {},
		onPathStateChange = () => {},
	}: {
		client?: import("@/lib/tauri").TauriClient;
		workspaceReadContext: WorkspaceReadContext;
		initialNamespaces: string[];
		initialKinds: ResourceKindSelection[];
		availableKinds?: ResourceKindSelection[];
		kindScopeLocked?: boolean;
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
		onArgoApplicationInspect?: (app: ArgoApplicationSummary) => void;
		onPathStateChange?: (state: PathStateResourceBrowserState) => void;
	} = $props();
	// svelte-ignore state_referenced_locally -- The mounted browser owns one fixed transport.
	const client = providedClient ?? createTauriClient();

	let selectedNamespaces = $state<string[]>([]);
	let selectedKinds = $state<ResourceKindSelection[]>([]);
	let appliedScopeKey = $state("");
	let search = $state("");
	let filterSearch = $state("");
	let gitOpsFilter = $state("");
	let healthFilter = $state<HealthFilter>("all");
	let argoResourceFilter = $state<ArgoResourceFilter>("none");
	let sortColumn = $state<ResourceSortColumn>("name");
	let sortDesc = $state(false);
	let pageIndex = $state(0);
	let scopeEditorOpen = $state(false);
	let collapsedGroups = $state<Set<string>>(new Set());
	let selectedTopologyNodeId = $state<string | null>(null);
	let topologyMode = $state<TopologyMode>("ownership");
	let appliedAvailableKinds = $state.raw<ResourceKindSelection[]>([]);
	// svelte-ignore state_referenced_locally
	let mapPanelOpen = $state(
		initialOwnershipMapOpen(initialPathState, getSettingsSnapshot().showOwnershipMapByDefault),
	);
	let hideHistoricalReplicaSets = $state(false);
	let OwnershipMapComponent = $state<typeof import("./OwnershipMap.svelte").default | null>(null);
	let ownershipMapLoadError = $state<unknown>(null);
	let tablePanelOpen = $state(true);
	let appliedTargetResourceKey = $state("");
	let appliedMetricsScopeKey = $state("");
	let appliedSelectionScrollKey = $state("");
	let metricsQueryReady = $state(false);
	let tableViewportElement = $state<HTMLDivElement | null>(null);
	let initialPathStateConsumed = $state(false);
	const showFullTopologyOnSelection = $derived($settingsStore.showFullTopologyOnSelection);

	$effect(() => {
		if (
			!shouldLoadOwnershipMap(
				mapPanelOpen,
				Boolean(OwnershipMapComponent),
				Boolean(ownershipMapLoadError),
			)
		) return;
		void loadOwnershipMap()
			.then((module) => {
				OwnershipMapComponent = module.default;
			})
			.catch((cause: unknown) => {
				ownershipMapLoadError = cause;
			});
	});

	function retryOwnershipMap() {
		ownershipMapLoadError = null;
		void retryOwnershipMapLoad()
			.then((module) => {
				OwnershipMapComponent = module.default;
			})
			.catch((cause: unknown) => {
				ownershipMapLoadError = cause;
			});
	}

	const incomingScopeKey = $derived(
		JSON.stringify({
			namespaces: [...initialNamespaces].sort(),
			kinds: initialKinds.map(kindKey).sort(),
			search: initialSearch,
			gitOpsFilter: initialGitOpsFilter,
			healthFilter: initialHealthFilter,
			argoApplication: gitOpsFocusApplication
				? `${gitOpsFocusApplication.namespace ?? ""}:${gitOpsFocusApplication.name}`
				: null,
		}),
	);
	$effect(() => {
		if (appliedScopeKey === incomingScopeKey) return;
		const pathState = initialPathStateConsumed || kindScopeLocked ? null : initialPathState;
		selectedNamespaces = pathState ? [...pathState.selectedNamespaces] : [...initialNamespaces];
		selectedKinds = pathState ? [...pathState.selectedKinds] : [...initialKinds];
		appliedScopeKey = incomingScopeKey;
		appliedAvailableKinds = initialKinds;
		search = pathState?.search ?? initialSearch;
		filterSearch = search;
		gitOpsFilter = pathState?.gitOpsFilter ?? initialGitOpsFilter;
		healthFilter = pathState?.healthFilter ?? initialHealthFilter;
		argoResourceFilter = "none";
		sortColumn = pathState?.sortColumn ?? "name";
		sortDesc = pathState?.sortDesc ?? false;
		pageIndex = pathState?.pageIndex ?? 0;
		scopeEditorOpen = pathState?.scopeEditorOpen ?? false;
		collapsedGroups = new Set(pathState?.collapsedGroups ?? []);
		selectedTopologyNodeId = pathState?.selectedTopologyNodeId ?? null;
		topologyMode = pathState?.topologyMode ?? "ownership";
		mapPanelOpen = initialOwnershipMapOpen(pathState, getSettingsSnapshot().showOwnershipMapByDefault);
		tablePanelOpen = pathState?.tablePanelOpen ?? true;
		initialPathStateConsumed = true;
	});
	/** Selects kinds that became available since they were last applied. */
	function selectNewlyAvailableKinds() {
		if (sameKinds(appliedAvailableKinds, availableKinds)) return;
		const additions = newlyAvailableKinds(availableKinds, appliedAvailableKinds, selectedKinds);
		appliedAvailableKinds = availableKinds;
		if (additions.length > 0) selectedKinds = [...selectedKinds, ...additions];
	}
	$effect(() => {
		if (appliedScopeKey !== "") selectNewlyAvailableKinds();
	});

	const clusterContext = $derived(workspaceReadContext.clusterContext);
	const sourceReady = $derived(workspaceReadContext.sourceReady);
	const kubeconfigSourceKey = $derived(workspaceReadContext.kubeconfigSourceKey);
	const fetchKeys = $derived(kindFetchKeys(selectedNamespaces, selectedKinds));
	const readSpecs = $derived(
		buildResourceBrowserReadSpecs({
			client,
			clusterContext,
			kubeconfigSourceKey,
			fetchKeys,
			namespaces: selectedNamespaces,
			topologyMode,
			mapPanelOpen,
			sourceReady,
			customResourcesEnabled,
		}),
	);
	const focusedArgo = argoApplicationInspection(client, () => ({
		readContext: workspaceReadContext,
		application: gitOpsFocusApplication,
	}));
	const workspaceId = $derived(workspaceReadContext.workspaceId);
	const live = liveViewRefresh(client, () =>
		resourceBrowserLiveView({ workspaceId, clusterContext, kubeconfigSourceKey, sourceReady }, readSpecs, {
			fetchKeys,
			namespaces: selectedNamespaces,
			mapPanelOpen,
			focusedApplication: gitOpsFocusApplication,
		}),
	);
	const namespacesQuery = createQuery<NamespaceSummary[]>(() => ({
		queryKey: readSpecs.namespacesQueryKey,
		queryFn: () => listNamespaces(client, clusterContext, kubeconfigSourceKey),
		enabled: readSpecs.namespacesEnabled,
		staleTime: 30_000,
		retry: false,
	}));
	const resourceKindsQuery = createQuery<DiscoveredResourceKind[]>(() => ({
		queryKey: readSpecs.resourceKindsQueryKey,
		queryFn: () => listResourceKinds(client, clusterContext, kubeconfigSourceKey),
		enabled: readSpecs.resourceKindsEnabled,
		staleTime: 30_000,
		retry: false,
	}));
	$effect(() => {
		if (sourceReady && namespacesQuery.isSuccess && resourceKindsQuery.isSuccess) {
			markStartup("base-scope-ready");
		}
	});
	const topologyFitViewKey = $derived(readSpecs.topologyFitViewKey);
	const resourcesQuery = createQuery<ResourceScopeResult>(() => readSpecs.resources);
	$effect(() => {
		if (kindScopeLocked) {
			if (selectedKinds.length !== initialKinds.length ||
				selectedKinds.some((kind, index) => kindKey(kind) !== kindKey(initialKinds[index]))) {
				selectedKinds = [...initialKinds];
			}
			appliedAvailableKinds = availableKinds;
			return;
		}
		if (!customResourcesEnabled) {
			const builtinKinds = selectedKinds.filter((kind) => !isCustomResourceKind(kind));
			if (builtinKinds.length !== selectedKinds.length) selectedKinds = builtinKinds;
			appliedAvailableKinds = availableKinds;
			return;
		}
		if (appliedScopeKey !== "" && resourcesQuery.isSuccess && !resourcesQuery.isPlaceholderData) {
			selectNewlyAvailableKinds();
		}
	});
	const topologyQuery = createQuery<ResourceTopology>(() => readSpecs.topology);
	const updatesCurrent = $derived(live.state.status === "connected" && !live.state.catchingUp && !live.state.reloadError
		&& !resourcesQuery.isError && !resourcesQuery.isPlaceholderData
		&& (!mapPanelOpen || (!topologyQuery.isError && !topologyQuery.isPlaceholderData)));
	$effect(() => {
		const metricsScopeKey = JSON.stringify(readSpecs.metrics.queryKey);
		if (appliedMetricsScopeKey === metricsScopeKey) return;
		appliedMetricsScopeKey = metricsScopeKey;
		metricsQueryReady = false;
	});
	$effect(() => {
		const rowCount = resourcesQuery.data?.resources.length ?? 0;
		if (
			metricsQueryReady ||
			!clusterContext ||
			!resourcesQuery.isSuccess ||
			resourcesQuery.isPlaceholderData ||
			(mapPanelOpen && topologyQuery.isPending) ||
			!globalThis.window
		) return;
		diagnosticLog("resources.metrics.defer", {
			ms: BACKGROUND_METRICS_DELAY_MS,
			rows: rowCount,
			mapOpen: mapPanelOpen,
		});
		const timer = window.setTimeout(() => {
			diagnosticLog("resources.metrics.enable", {
				rows: rowCount,
				mapOpen: mapPanelOpen,
			});
			metricsQueryReady = true;
		}, BACKGROUND_METRICS_DELAY_MS);
		return () => window.clearTimeout(timer);
	});
	const metricsQuery = createQuery<ResourceMetricsSummary>(() => ({
		...readSpecs.metrics,
		enabled: metricsQueryReady && Boolean(clusterContext),
	}));

	const namespaceOptions = $derived(namespacesQuery.data ?? []);
	const kindOptions = $derived(
		kindScopeLocked
			? [...initialKinds]
			: allKindOptions(customResourcesEnabled ? (resourceKindsQuery.data ?? []) : []),
	);
	const selectedNamespaceSet = $derived(new Set(selectedNamespaces));
	const selectedKindSet = $derived(new Set(selectedKinds.map(kindKey)));
	const metricsIndex = $derived(resourceMetricIndex(metricsQuery.data));
	const scopedRows = $derived(
		filterResourcesByKinds(resourcesQuery.data?.resources ?? [], selectedKinds),
	);
	const rowsWithMetrics = $derived(
		mergeResourceMetrics(scopedRows, metricsQuery.data, metricsIndex),
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
					rowsWithMetrics,
					focusedArgoResources,
					argoResourceFilter,
				)
			: rowsWithMetrics,
	);
	let firstResourceRowsMarked = false;
	$effect(() => {
		if (firstResourceRowsMarked) return;
		if (tablePanelOpen && resourcesQuery.isSuccess && !resourcesQuery.isPlaceholderData && tableModel.entries.some((entry) => entry.type === "resource")) {
			markStartup("first-resource-rows");
			firstResourceRowsMarked = true;
		}
	});
	const tableProjection = $derived(buildResourceTableProjection(tableRows));
	$effect(() => {
		const value = search;
		const frame = requestAnimationFrame(() => {
			if (filterSearch === value) return;
			filterSearch = value;
			pageIndex = 0;
		});
		return () => cancelAnimationFrame(frame);
	});
	const tableDisplay = $derived(
		buildResourceTableDisplay(
			tableProjection,
			{
				search: filterSearch,
				gitOpsFilter,
				healthFilter,
				sort: { id: sortColumn, desc: sortDesc },
				preferredGitOpsResourceKeys: gitOpsFocusApplication
					? focusedArgoResourceKeys
					: undefined,
				kindOrder: resourceKindOrder($settingsStore.resourceKindLanes),
			},
		),
	);
	const tablePage = $derived(buildResourceTablePage(tableDisplay, pageIndex));
	const buildPageEntries = createResourceTableEntryBuilder();
	const tableModel = $derived({
		...tablePage,
		entries: buildPageEntries(tablePage, collapsedGroups, selectedResource),
	});
	const filterTableTopology = createTableTopologyFilter();
	const topologyWithMetrics = $derived(
		mergeTopologyMetrics(
			filterTableTopology(
				topologyQuery.data,
				tableDisplay.filteredRows,
				hideHistoricalReplicaSets,
			),
			metricsQuery.data,
			metricsIndex,
		),
	);
	const pinnedResourceKeySet = $derived(new Set(pinnedResourceKeys));
	const tableVisibleColumnCount = $derived(
		6 +
			Number(tableModel.columnVisibility.ready) +
			Number(tableModel.columnVisibility.restarts) +
			Number(tableModel.columnVisibility.cpu) +
			Number(tableModel.columnVisibility.memory) +
			Number(tableModel.columnVisibility.gitOps),
	);
	const tableMinWidth = $derived(
		250 +
			130 +
			105 +
			95 +
			(tableModel.columnVisibility.ready ? 70 : 0) +
			(tableModel.columnVisibility.restarts ? 75 : 0) +
			(tableModel.columnVisibility.cpu ? 65 : 0) +
			(tableModel.columnVisibility.memory ? 85 : 0) +
			(tableModel.columnVisibility.gitOps ? 110 : 0) +
			65 +
			40,
	);
	const tableStickyMeasureKey = $derived(
		`${tablePanelOpen}:${tableModel.entries.length}:${tableVisibleColumnCount}`,
	);
	const selectedResourceKey = $derived(
		selectedResource ? resourceKey(selectedResource) : "",
	);
	const selectedPageRow = $derived(
		selectedResource
			? tableModel.pageRows[findResourceIndex(tableModel.pageRows, selectedResource, (row) => row)]
			: undefined,
	);
	const inspectorOpen = $derived(Boolean(selectedResource));
	const syncedTopologyNodeId = $derived(
		resolveSyncedTopologyNodeId({
			selectedTopologyNodeId,
			selectedResource,
			topologyNodes: topologyWithMetrics?.nodes,
		}),
	);
	const targetResourceKey = $derived(
		targetResource ? resourceKey(targetResource) : "",
	);
	const metricsMessage = $derived(
		metricsQuery.isError
			? "metrics API unavailable"
			: describeMetricsAvailability(metricsQuery.data?.availability),
	);
	const loading = $derived(
		Boolean(clusterContext) &&
			fetchKeys.length > 0 &&
			(!sourceReady || (resourcesQuery.isPending && !resourcesQuery.isPlaceholderData)),
	);
	const resourceError = $derived(
		workspaceReadContext.sourceError
			? workspaceReadContext.sourceError
			: resourcesQuery.isError
				? resourcesQuery.error
				: null,
	);

	$effect(() => {
		onPathStateChange({
			selectedNamespaces,
			selectedKinds,
			search,
			gitOpsFilter,
			healthFilter,
			sortColumn,
			sortDesc,
			pageIndex: tableModel.safePageIndex,
			scopeEditorOpen,
			collapsedGroups: [...collapsedGroups],
			topologyMode,
			selectedTopologyNodeId,
			mapPanelOpen,
			tablePanelOpen,
		});
	});

	$effect(() => {
		const measureKey = tableStickyMeasureKey;
		const viewport = tableViewportElement;
		if (!tablePanelOpen || !viewport || !measureKey) return;
		const measure = () => measureTableStickyOffsets(viewport);
		const frame = window.requestAnimationFrame(measure);
		const resizeObserver = "ResizeObserver" in globalThis
			? new ResizeObserver(measure)
			: null;
		resizeObserver?.observe(viewport);
		return () => {
			window.cancelAnimationFrame(frame);
			resizeObserver?.disconnect();
		};
	});

	$effect(() => {
		const viewport = tableViewportElement;
		const scrollKey = selectedResourceKey;
		if (!scrollKey) {
			appliedSelectionScrollKey = "";
			return;
		}
		if (appliedSelectionScrollKey === scrollKey) return;
		const entries = tableModel.entries;
		if (!tablePanelOpen || !viewport || entries.length === 0) {
			return;
		}
		appliedSelectionScrollKey = scrollKey;
		let secondFrame: number | null = null;
		const firstFrame = window.requestAnimationFrame(() => {
			scrollSelectedTableRowIntoView(viewport);
			secondFrame = window.requestAnimationFrame(() => scrollSelectedTableRowIntoView(viewport));
		});
		return () => {
			window.cancelAnimationFrame(firstFrame);
			if (secondFrame !== null) window.cancelAnimationFrame(secondFrame);
		};
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
		selectedTopologyNodeId = null;
		pageIndex = Math.max(0, Math.floor(rowIndex / PAGE_SIZE));
		appliedTargetResourceKey = targetResourceKey;
	});

	function toggleNamespace(namespace: string, checked: boolean) {
		selectedNamespaces = nextNamespaceSelection(
			selectedNamespaces,
			namespaceOptions.map((option) => option.name),
			namespace,
			checked,
		);
		pageIndex = 0;
		onResourceClose();
		selectedTopologyNodeId = null;
	}

	function toggleKind(kind: ResourceKindSelection, checked: boolean) {
		const key = kindKey(kind);
		selectedKinds = checked
			? [...selectedKinds, kind]
			: selectedKinds.filter((item) => kindKey(item) !== key);
		pageIndex = 0;
		onResourceClose();
		selectedTopologyNodeId = null;
	}

	function toggleSort(column: ResourceSortColumn) {
		if (sortColumn === column) {
			sortDesc = !sortDesc;
		} else {
			sortColumn = column;
			sortDesc = false;
		}
	}

	function toggleGroup(key: string) {
		const next = new Set(collapsedGroups);
		if (next.has(key)) next.delete(key);
		else next.add(key);
		collapsedGroups = next;
	}

	function selectHealth(filter: HealthFilter) {
		healthFilter = filter;
		pageIndex = 0;
	}

	function selectArgoResource(filter: ArgoResourceFilter) {
		argoResourceFilter = argoResourceFilter === filter ? "none" : filter;
		pageIndex = 0;
	}

	function clearFilters() {
		search = "";
		gitOpsFilter = "";
		healthFilter = "all";
		argoResourceFilter = "none";
		pageIndex = 0;
	}

	function setGitOpsFilter(value: string) {
		gitOpsFilter = value === "__all" ? "" : value;
		pageIndex = 0;
	}

	function selectResource(resource: ResourceSummary) {
		selectedTopologyNodeId = null;
		onResourceSelect(resource, "explicit");
	}

	function selectTopologyResource(nodeId: string, resource: ResourceSummary | null) {
		selectedTopologyNodeId = nodeId;
		if (resource) onResourceSelect(resource, "explicit");
	}

	function closeMapPanel() {
		mapPanelOpen = false;
		selectedTopologyNodeId = null;
	}

	function scrollSelectedTableRowIntoView(viewport: HTMLDivElement) {
		const selectedRow = viewport.querySelector<HTMLElement>(
			"tr[data-resource-selected='true']",
		);
		selectedRow?.scrollIntoView({ block: "center", inline: "nearest" });
	}

	function measureTableStickyOffsets(viewport: HTMLDivElement) {
		const headerRow = viewport.querySelector("thead tr");
		const appGroupCell = viewport.querySelector('[data-sticky="app-group"]');
		const headerHeight = headerRow ? Math.round(headerRow.getBoundingClientRect().height) : 0;
		const appGroupHeight = appGroupCell
			? Math.round(appGroupCell.getBoundingClientRect().height)
			: 0;
		viewport.style.setProperty("--sticky-app-top", `${Math.max(headerHeight - 1, 0)}px`);
		viewport.style.setProperty(
			"--sticky-type-top",
			`${Math.max(headerHeight + appGroupHeight - 2, 0)}px`,
		);
	}

	function statusBadgeVariant(tone: ChipVariant) {
		return STATUS_BADGE_STYLES[tone].variant;
	}

	function statusBadgeClass(tone: ChipVariant) {
		return `rounded-full px-2 py-0 text-[0.6875rem] shadow-none ${STATUS_BADGE_STYLES[tone].className}`;
	}
</script>

{#snippet refreshControls()}
	{#key live.key}
		<ResourceLiveStatusMenu {live} disabled={!sourceReady || !clusterContext} />
	{/key}
{/snippet}

<div class="flex h-full min-h-0 min-w-0 flex-col gap-3">
	{#if resourcesQuery.isSuccess && !resourcesQuery.isPlaceholderData}
		<ResourceScopeWarnings failures={resourcesQuery.data?.failures ?? []} />
	{/if}
	{#if !clusterContext || fetchKeys.length === 0 || resourceError || loading}
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
	{:else if resourceError && (!resourcesQuery.data || workspaceReadContext.sourceError)}
		<FriendlyError
			error={resourceError}
			context={{ operation: "resourcesLoad", fallbackTitle: "Failed to load resources" }}
		/>
	{:else if loading}
		<div class="flex min-h-52 items-center justify-center gap-2 text-xs text-muted-foreground">
			<Spinner />
			<span>Loading resources</span>
		</div>
	{:else}
		<ResourceBrowserTopBar
			{kindScopeLocked}
			selectedNamespaces={selectedNamespaces}
			selectedKinds={selectedKinds}
			namespaceOptions={namespaceOptions}
			kindOptions={kindOptions}
			selectedNamespaceSet={selectedNamespaceSet}
			selectedKindSet={selectedKindSet}
			healthSummary={tableModel.healthSummary}
			{healthFilter}
			bind:search
			{gitOpsFilter}
			gitOpsFilters={tableModel.gitOpsFilters}
			argoSummary={focusedArgoFilterSummary}
			argoFilter={argoResourceFilter}
			{hideHistoricalReplicaSets}
			{metricsMessage}
			{customResourcesStatus}
			{refreshControls}
			onAllNamespacesSelect={() => {
				selectedNamespaces = [];
				pageIndex = 0;
				onResourceClose();
				selectedTopologyNodeId = null;
			}}
			onAllKindsSelect={() => {
				selectedKinds = [...kindOptions];
				pageIndex = 0;
				onResourceClose();
				selectedTopologyNodeId = null;
			}}
			onNamespaceToggle={toggleNamespace}
			onKindToggle={toggleKind}
			onHealthSelect={selectHealth}
			onArgoFilterSelect={selectArgoResource}
			onHideHistoricalReplicaSetsChange={(hide) => (hideHistoricalReplicaSets = hide)}
			onGitOpsFilterChange={setGitOpsFilter}
			onClearFilters={clearFilters}
		/>

		{#if live.state.reloadError}
			<details class="rounded-md border border-amber-500/30 text-xs text-amber-700 dark:text-amber-300" data-refresh-error>
				<summary class="cursor-pointer px-3 py-2"><span role="status">Refresh failed. Showing last observed state. Activity indicators are paused until live updates recover.</span></summary>
				<div class="px-3 pb-3"><FriendlyError error={live.state.reloadError} context={{ operation: "resourcesLoad", fallbackTitle: "Resource refresh failed. Retrying." }} /></div>
			</details>
		{:else if !updatesCurrent && (resourcesQuery.data || topologyQuery.data)}
			<p role="status" class="rounded-md border border-amber-500/30 px-3 py-2 text-xs text-amber-700 dark:text-amber-300">
				Showing last observed state. Activity indicators are paused until live updates recover.
			</p>
		{/if}
		<div
			class={inspectorOpen && mapPanelOpen && tablePanelOpen
				? "grid min-h-0 min-w-0 flex-1 grid-cols-1 grid-rows-[minmax(400px,1fr)_minmax(400px,1fr)] gap-3"
				: mapPanelOpen && tablePanelOpen
					? "grid min-h-0 min-w-0 flex-1 gap-3 min-[1101px]:grid-cols-[minmax(0,0.42fr)_minmax(0,0.58fr)]"
				: mapPanelOpen
					? "grid min-h-0 min-w-0 flex-1 grid-cols-[minmax(0,1fr)_3rem] gap-3"
					: tablePanelOpen
						? "grid min-h-0 min-w-0 flex-1 grid-cols-[3rem_minmax(0,1fr)] gap-3"
						: "grid min-h-0 min-w-0 flex-1 grid-cols-[3rem_3rem] gap-3"}
		>
			{#if mapPanelOpen}
				<div class="h-full min-h-[400px] min-w-0">
					{#if OwnershipMapComponent}
						<OwnershipMapComponent
							topology={topologyWithMetrics}
							{updatesCurrent}
							isLoading={topologyQuery.isPending && !topologyQuery.data}
							isError={topologyQuery.isError && !topologyQuery.data}
							error={topologyQuery.error}
							mode={topologyMode}
							selectedNodeId={syncedTopologyNodeId}
							{showFullTopologyOnSelection}
							fitViewKey={topologyFitViewKey}
							onModeChange={(mode) => {
								topologyMode = mode;
								selectedTopologyNodeId = null;
							}}
							onNodeSelect={selectTopologyResource}
							onMapToggle={closeMapPanel}
						/>
					{:else if ownershipMapLoadError}
						<div class="flex h-full items-center justify-center gap-2 text-xs text-muted-foreground">
							<span>Failed to load ownership map.</span>
							<Button type="button" variant="outline" size="sm" onclick={retryOwnershipMap}>Retry</Button>
						</div>
					{:else}
						<div class="flex h-full items-center justify-center gap-2 text-xs text-muted-foreground">
							<Spinner />
							<span>Loading ownership map</span>
						</div>
					{/if}
				</div>
			{:else}
				<aside
					class="flex h-full min-h-[400px] w-12 shrink-0 flex-col items-center overflow-hidden rounded-lg border bg-surface-1 shadow-sm"
				>
					<div class="flex w-full justify-center border-b p-2">
						<Button
							type="button"
							variant="ghost"
							size="icon"
							class="size-7"
							onclick={() => (mapPanelOpen = true)}
							aria-label="Show ownership map"
						>
							<PanelRightOpen />
						</Button>
					</div>
					<Button
						type="button"
						variant="ghost"
						class="h-auto min-h-0 w-full flex-1 flex-col gap-2 rounded-none border-0 px-2 py-3 text-muted-foreground hover:bg-transparent hover:text-foreground"
						onclick={() => (mapPanelOpen = true)}
						aria-label="Show ownership map"
					>
						<GitBranch class="size-4 shrink-0" />
						<span class="[writing-mode:vertical-rl] text-xs font-semibold">Map</span>
					</Button>
				</aside>
			{/if}

			{#if tablePanelOpen}
				<aside
					class="flex h-full min-h-[400px] min-w-0 flex-col overflow-hidden rounded-lg border bg-surface-1 shadow-sm"
				>
					<div class="flex items-start justify-between gap-2 border-b px-3 py-2">
						<div class="min-w-0">
							<div class="flex min-w-0 items-center gap-2">
								<Table2 class="size-4 shrink-0 text-muted-foreground" />
								<h2 class="truncate text-sm font-semibold">Resource Table</h2>
							</div>
							<p class="mt-0.5 text-xs text-muted-foreground">
								{tableModel.totalRows} resources · page {tableModel.safePageIndex + 1} of {tableModel.pageCount}
							</p>
						</div>
						<Button
							type="button"
							variant="ghost"
							size="icon"
							class="size-7"
							onclick={() => (tablePanelOpen = false)}
							aria-label="Collapse resource table"
						>
							<PanelRightClose />
						</Button>
					</div>
					<div
						bind:this={tableViewportElement}
						class="scrollbar-classic h-full min-h-0 flex-1 overflow-auto [&_[data-slot=table-container]]:overflow-visible"
					>
						<Table aria-label={`${title} resource table`} class={TABLE_CLASS} style={`min-width: ${tableMinWidth}px;`}>
							<colgroup>
								<col />
								<col style="width: 130px;" />
								<col style="width: 105px;" />
								<col style="width: 95px;" />
								{#if tableModel.columnVisibility.ready}<col style="width: 70px;" />{/if}
								{#if tableModel.columnVisibility.restarts}<col style="width: 75px;" />{/if}
								{#if tableModel.columnVisibility.cpu}<col style="width: 65px;" />{/if}
								{#if tableModel.columnVisibility.memory}<col style="width: 85px;" />{/if}
								{#if tableModel.columnVisibility.gitOps}<col style="width: 110px;" />{/if}
								<col style="width: 65px;" />
								<col style="width: 40px;" />
							</colgroup>
							<TableHeader>
								<TableRow>
									<TableHead>{@render SortButton("name", "Name")}</TableHead>
									<TableHead>{@render SortButton("status", "Status")}</TableHead>
									<TableHead>{@render SortButton("namespace", "Namespace")}</TableHead>
									<TableHead>{@render SortButton("kind", "Kind")}</TableHead>
									{#if tableModel.columnVisibility.ready}<TableHead>{@render SortButton("ready", "Ready")}</TableHead>{/if}
									{#if tableModel.columnVisibility.restarts}<TableHead>{@render SortButton("restarts", "Restarts")}</TableHead>{/if}
									{#if tableModel.columnVisibility.cpu}<TableHead>{@render SortButton("cpu", "CPU", CPU_USAGE_DESCRIPTION)}</TableHead>{/if}
									{#if tableModel.columnVisibility.memory}<TableHead>{@render SortButton("memory", "Memory", MEMORY_USAGE_DESCRIPTION)}</TableHead>{/if}
									{#if tableModel.columnVisibility.gitOps}<TableHead>Owner</TableHead>{/if}
									<TableHead>{@render SortButton("age", "Age")}</TableHead>
									<TableHead><span class="sr-only">Pin</span></TableHead>
								</TableRow>
							</TableHeader>
							<TableBody>
								{#if tableModel.entries.length === 0}
									<TableRow>
										<TableCell colspan={tableVisibleColumnCount} class={EMPTY_PAGE_CLASS}>
											No resources match current scope and filters.
										</TableCell>
									</TableRow>
								{:else}
									{#each tableModel.entries as entry (entry.key)}
										{#if entry.type === "group"}
											{@const visual = getResourceGroupVisual(entry.label)}
											{@const GroupIcon = visual.icon}
											<TableRow class="text-xs font-bold text-primary hover:bg-transparent">
												<TableCell
													colspan={tableVisibleColumnCount}
													data-sticky="app-group"
													class={cnfast("sticky z-20 !p-0 bg-background", STICKY_APP_GROUP_TOP)}
												>
							<Button
								type="button"
								variant="ghost"
								class="h-auto w-full justify-start rounded-none border-0 bg-muted/50 px-3 py-2 text-left text-inherit hover:bg-muted/60"
								onclick={() => toggleGroup(entry.key)}
								aria-expanded={!entry.collapsed}
							>
														{#if entry.collapsed}
															<ChevronRight class="size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
														{:else}
															<ChevronDown class="size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
														{/if}
														<GroupIcon class={cnfast("size-3.5 shrink-0", visual.className)} aria-hidden="true" />
														<span class="text-muted-foreground">{entry.label}</span>
														<small class="text-[0.6875rem] font-medium text-muted-foreground">
									{entry.count} resources on this page
								</small>
							</Button>
												</TableCell>
											</TableRow>
										{:else if entry.type === "type"}
											{@const visual = getResourceKindVisual(entry.kind)}
											{@const TypeIcon = visual.icon}
											<TableRow class="text-[0.72rem] font-bold uppercase text-foreground hover:bg-transparent">
												<TableCell
													colspan={tableVisibleColumnCount}
													class={cnfast("sticky z-10 !p-0 bg-card", STICKY_TYPE_GROUP_TOP)}
												>
							<Button
								type="button"
								variant="ghost"
								class={cnfast(
									"h-auto w-full justify-start rounded-none border-0 bg-card py-1.5 pr-3 text-left text-[0.6875rem] text-inherit hover:bg-muted/40",
									tableModel.groupedByGitOps ? "pl-6" : "pl-3",
								)}
								onclick={() => toggleGroup(entry.key)}
														aria-expanded={!entry.collapsed}
													>
														{#if entry.collapsed}
															<ChevronRight class="size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
														{:else}
															<ChevronDown class="size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
														{/if}
														<TypeIcon class={cnfast("size-3.5 shrink-0", visual.className)} aria-hidden="true" />
														<span>{entry.label}</span>
														<small class="text-[0.625rem] font-medium normal-case text-muted-foreground">
									{entry.count} on this page
								</small>
							</Button>
												</TableCell>
											</TableRow>
										{:else}
											{@const row = entry.resource}
											{@const rowSelected = row === selectedPageRow}
											<TableRow
												data-resource-selected={rowSelected ? "true" : undefined}
												class={cnfast(ROW_CLASS, "cursor-pointer", rowSelected && SELECTED_ROW_CLASS)}
												onclick={(event: MouseEvent) => {
													if (event.target instanceof Element && event.target.closest("button")) return;
													selectResource(row);
												}}
											>
												<TableCell class="font-medium">
													<CopyableText
														value={row.name}
														label="resource name"
														onActivate={() => selectResource(row)}
														actionLabel={`Open resource ${row.name}`}
														active={rowSelected}
													/>
												</TableCell>
												<TableCell>
												<ResourceTableStatus resource={row} active={updatesCurrent} />
												</TableCell>
											<TableCell>{row.namespace ?? EMPTY_CELL}</TableCell>
											<TableCell>{row.kind}</TableCell>
												{#if tableModel.columnVisibility.ready}
													<TableCell>
														{@const ready = resourceReadyChip(row)}
														{#if ready}
															<Badge variant={statusBadgeVariant(ready.tone)} class={statusBadgeClass(ready.tone)}>
																{ready.value}
															</Badge>
														{:else}
															<span title={READINESS_NOT_REPORTED} aria-label={READINESS_NOT_REPORTED}>{row.ready ?? EMPTY_CELL}</span>
														{/if}
													</TableCell>
												{/if}
												{#if tableModel.columnVisibility.restarts}
													<TableCell>
														{#if row.restarts === undefined || row.restarts === null}
															<span class="flex justify-center" title={RESTART_COUNT_NOT_REPORTED} aria-label={RESTART_COUNT_NOT_REPORTED}>{EMPTY_CELL}</span>
														{:else if row.restarts === 0}
															<span class="flex justify-center">{row.restarts}</span>
														{:else}
															{@const restartTone = row.restarts > 5 ? "error" : "warning"}
															<span class="flex justify-center">
																<Badge variant={statusBadgeVariant(restartTone)} class={statusBadgeClass(restartTone)}>
																	{row.restarts}
																</Badge>
															</span>
														{/if}
													</TableCell>
												{/if}
												{#if tableModel.columnVisibility.cpu}
														<TableCell title={CPU_USAGE_DESCRIPTION} aria-label={`CPU usage ${formatCpuMillicores(row.metrics?.cpuMillicores)}`}>{formatCpuMillicores(row.metrics?.cpuMillicores)}</TableCell>
												{/if}
												{#if tableModel.columnVisibility.memory}
														<TableCell title={MEMORY_USAGE_DESCRIPTION} aria-label={`Memory usage ${formatMemoryBytes(row.metrics?.memoryBytes)}`}>{formatMemoryBytes(row.metrics?.memoryBytes)}</TableCell>
												{/if}
												{#if tableModel.columnVisibility.gitOps}
													<TableCell>
														<span class="block min-w-0 truncate">
															{gitOpsOwnership(row)?.ownerName ?? row.helmRelease ?? EMPTY_CELL}
														</span>
													</TableCell>
												{/if}
												<TableCell><TimestampText value={row.createdAt} relative={row.age} /></TableCell>
												<TableCell>
													<Button
														type="button"
														variant="ghost"
														size="icon"
														class="size-7"
														aria-label={`${pinnedResourceKeySet.has(resourceKey(row)) ? "Unpin" : "Pin"} ${row.kind} ${row.name}`}
														onclick={(event: MouseEvent) => {
															event.stopPropagation();
															onResourcePinToggle(row);
														}}
														onkeydown={(event: KeyboardEvent) => event.stopPropagation()}
													>
														<Pin
															class={pinnedResourceKeySet.has(resourceKey(row)) ? "fill-current" : ""}
														/>
													</Button>
												</TableCell>
											</TableRow>
										{/if}
									{/each}
								{/if}
							</TableBody>
						</Table>
					</div>
					<div class="grid grid-cols-[auto_1fr_auto] items-center gap-2 border-t px-3 py-2 text-xs text-muted-foreground">
						<Button
							variant="outline"
							size="sm"
							disabled={tableModel.safePageIndex === 0}
							onclick={() => (pageIndex = Math.max(0, tableModel.safePageIndex - 1))}
						>
							Previous
						</Button>
						<div class="grid grid-cols-2 text-center">
							<span>{tableModel.totalRows} total rows</span>
							<span>Page {tableModel.safePageIndex + 1} of {tableModel.pageCount}</span>
						</div>
						<Button
							variant="outline"
							size="sm"
							disabled={tableModel.safePageIndex >= tableModel.pageCount - 1}
							onclick={() =>
								(pageIndex = Math.min(
									tableModel.pageCount - 1,
									tableModel.safePageIndex + 1,
								))}
						>
							Next
						</Button>
					</div>
				</aside>
			{:else}
				<aside
					class="flex h-full min-h-[400px] w-12 shrink-0 flex-col items-center overflow-hidden rounded-lg border bg-surface-1 shadow-sm"
				>
					<div class="flex w-full justify-center border-b p-2">
						<Button
							type="button"
							variant="ghost"
							size="icon"
							class="size-7"
							onclick={() => (tablePanelOpen = true)}
							aria-label="Show resource table"
						>
							<PanelRightOpen />
						</Button>
					</div>
					<Button
						type="button"
						variant="ghost"
						class="h-auto min-h-0 w-full flex-1 flex-col gap-2 rounded-none border-0 px-2 py-3 text-muted-foreground hover:bg-transparent hover:text-foreground"
						onclick={() => (tablePanelOpen = true)}
						aria-label="Show resource table"
					>
						<Table2 class="size-4 shrink-0" />
						<span class="[writing-mode:vertical-rl] text-xs font-semibold">Table</span>
					</Button>
				</aside>
			{/if}
		</div>
	{/if}
</div>

{#snippet SortButton(column: ResourceSortColumn, label: string, description?: string)}
	<Button
		type="button"
		variant="ghost"
		size="xs"
		class="h-auto gap-1 border-0 bg-transparent p-0 text-left text-inherit hover:bg-transparent"
		onclick={() => toggleSort(column)}
		aria-label={`Sort by ${label}${description ? `. ${description}` : ""}`}
		title={description}
	>
		{label}
		{#if sortColumn === column && sortDesc}<ArrowDown class="size-3" aria-hidden="true" />{:else if sortColumn === column}<ArrowUp class="size-3" aria-hidden="true" />{:else}<ChevronsUpDown class="size-3" aria-hidden="true" />{/if}
	</Button>
{/snippet}
