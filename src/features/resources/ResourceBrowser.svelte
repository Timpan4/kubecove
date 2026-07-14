<script lang="ts">
	import { createQuery, useQueryClient } from "@tanstack/svelte-query";
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
	import { createCancellableRequest, createCancelScope } from "@/lib/cancellable-loads";
	import { diagnosticLog } from "@/lib/diagnostics";
	import { withForegroundLoad } from "@/lib/foreground-loading";
	import { STATUS_BADGE_STYLES } from "@/components/status-badge-styles";
	import { healthStatusVariant, syncStatusVariant, type ChipVariant } from "@/features/argo/status";
	import {
		describeMetricsAvailability,
		formatCpuMillicores,
		formatMemoryBytes,
		mergeResourceMetrics,
		mergeTopologyMetrics,
	} from "@/lib/resource-metrics";
	import { queryKeys } from "@/lib/queryKeys";
	import { cnfast } from "@/lib/utils";
	import type { ArgoApplicationSummary } from "@/lib/gitops-types";
	import { getSettingsSnapshot, settingsStore } from "@/lib/settings-store";
	import type { PathStateResourceBrowserState } from "@/lib/path-state";
	import {
		closeStreamChannel,
		cancelBackendRequests,
		createStreamChannel,
		createTauriClient,
		getKubeconfigSources,
		isAppError,
		listNamespaces,
		listResourceKinds,
		listResourceMetrics,
		listResourceTopology,
		startResourceWatch,
		stopStream,
	} from "@/lib/tauri";
	import type {
		DiscoveredResourceKind,
		KubeconfigSourcesSummary,
		NamespaceSummary,
		ResourceKindSelection,
		ResourceMetricsSummary,
		ResourceSummary,
		ResourceTopology,
		TopologyMode,
	} from "@/lib/types";
	import {
		EMPTY_PAGE_CLASS,
		PAGE_SIZE,
		ROW_CLASS,
		SELECTED_ROW_CLASS,
		STICKY_APP_GROUP_TOP,
		STICKY_TYPE_GROUP_TOP,
		TABLE_CLASS,
	} from "./constants";
	import {
		buildFetchKeys,
		resourceReadyChip,
		resourceIdentityKey,
		resourceSelectionKey,
		resourceStatusTone,
		shouldDropWarmupWatchEvent,
		topologyWatchKeys,
		watchKeysFromFetchKeys,
	} from "./helpers";
	import type { HealthFilter } from "./helpers";
	import { fetchResourcePage } from "./query";
	import OwnershipMap from "./OwnershipMap.svelte";
	import ResourceBrowserTopBar from "./ResourceBrowserTopBar.svelte";
	import {
		allKindOptions,
		buildResourceTableModel,
		kindSelectionKey,
		syncedTopologyNodeId as resolveSyncedTopologyNodeId,
		type ResourceSortColumn,
	} from "./resourceBrowserModel";

	const BACKGROUND_METRICS_DELAY_MS = 1_500;
	const EMPTY_CELL = "—";

	let {
		clusterContext,
		initialNamespaces,
		initialKinds,
		availableKinds = initialKinds,
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
		onPathStateChange = () => {},
	}: {
		clusterContext: string;
		initialNamespaces: string[];
		initialKinds: ResourceKindSelection[];
		availableKinds?: ResourceKindSelection[];
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
		onPathStateChange?: (state: PathStateResourceBrowserState) => void;
	} = $props();

	const client = createTauriClient();
	const queryClient = useQueryClient();
	let selectedNamespaces = $state<string[]>([]);
	let selectedKinds = $state<ResourceKindSelection[]>([]);
	let appliedScopeKey = $state("");
	let search = $state("");
	let gitOpsFilter = $state("");
	let healthFilter = $state<HealthFilter>("all");
	let sortColumn = $state<ResourceSortColumn>("name");
	let sortDesc = $state(false);
	let pageIndex = $state(0);
	let scopeEditorOpen = $state(false);
	let collapsedGroups = $state<Set<string>>(new Set());
	let selectedTopologyNodeId = $state<string | null>(null);
	let topologyMode = $state<TopologyMode>("ownership");
	let appliedAvailableKindsKey = $state("");
	let mapPanelOpen = $state(getSettingsSnapshot().showOwnershipMapByDefault);
	let tablePanelOpen = $state(true);
	let appliedTargetResourceKey = $state("");
	let appliedMetricsScopeKey = $state("");
	let appliedSelectionScrollKey = $state("");
	let metricsQueryReady = $state(false);
	let realtimeStatus = $state("idle");
	let realtimeMessage = $state("Realtime idle");
	let realtimeError = $state("");
	let tableViewportElement = $state<HTMLDivElement | null>(null);
	let initialPathStateConsumed = $state(false);
	const showFullTopologyOnSelection = $derived($settingsStore.showFullTopologyOnSelection);

	const incomingScopeKey = $derived(
		JSON.stringify({
			namespaces: [...initialNamespaces].sort(),
			kinds: initialKinds.map(kindSelectionKey).sort(),
			search: initialSearch,
			gitOpsFilter: initialGitOpsFilter,
			healthFilter: initialHealthFilter,
		}),
	);
	$effect(() => {
		if (appliedScopeKey === incomingScopeKey) return;
		const pathState = initialPathStateConsumed ? null : initialPathState;
		selectedNamespaces = pathState ? [...pathState.selectedNamespaces] : [...initialNamespaces];
		selectedKinds = pathState ? [...pathState.selectedKinds] : [...initialKinds];
		appliedScopeKey = incomingScopeKey;
		appliedAvailableKindsKey = initialKinds.map(kindSelectionKey).sort().join(",");
		search = pathState?.search ?? initialSearch;
		gitOpsFilter = pathState?.gitOpsFilter ?? initialGitOpsFilter;
		healthFilter = pathState?.healthFilter ?? initialHealthFilter;
		sortColumn = pathState?.sortColumn ?? "name";
		sortDesc = pathState?.sortDesc ?? false;
		pageIndex = pathState?.pageIndex ?? 0;
		scopeEditorOpen = pathState?.scopeEditorOpen ?? false;
		collapsedGroups = new Set(pathState?.collapsedGroups ?? []);
		selectedTopologyNodeId = pathState?.selectedTopologyNodeId ?? null;
		topologyMode = pathState?.topologyMode ?? "ownership";
		mapPanelOpen =
			pathState?.mapPanelOpen ?? getSettingsSnapshot().showOwnershipMapByDefault;
		tablePanelOpen = pathState?.tablePanelOpen ?? true;
		initialPathStateConsumed = true;
	});
	const availableKindsKey = $derived(
		availableKinds.map(kindSelectionKey).sort().join(","),
	);
	$effect(() => {
		if (appliedScopeKey === "" || appliedAvailableKindsKey === availableKindsKey) return;
		const previous = new Set(appliedAvailableKindsKey.split(",").filter(Boolean));
		const selected = new Set(selectedKinds.map(kindSelectionKey));
		const additions = availableKinds.filter((kind) => {
			const key = kindSelectionKey(kind);
			return !previous.has(key) && !selected.has(key);
		});
		appliedAvailableKindsKey = availableKindsKey;
		if (additions.length > 0) selectedKinds = [...selectedKinds, ...additions];
	});

	const sourceQuery = createQuery<KubeconfigSourcesSummary>(() => ({
		queryKey: ["kubeconfig-sources"] as const,
		queryFn: () => getKubeconfigSources(client),
		staleTime: 60_000,
	}));
	const sourceReady = $derived(sourceQuery.isSuccess || sourceQuery.isError);
	const kubeconfigSourceKey = $derived(sourceQuery.data?.sourceKey);
	const namespacesQuery = createQuery<NamespaceSummary[]>(() => ({
		queryKey: queryKeys.namespaces(clusterContext, kubeconfigSourceKey),
		queryFn: () => listNamespaces(client, clusterContext, kubeconfigSourceKey),
		enabled: Boolean(clusterContext) && sourceReady,
		staleTime: 30_000,
		retry: false,
	}));
	const resourceKindsQuery = createQuery<DiscoveredResourceKind[]>(() => ({
		queryKey: queryKeys.resourceKinds(clusterContext, kubeconfigSourceKey),
		queryFn: () => listResourceKinds(client, clusterContext, kubeconfigSourceKey),
		enabled: customResourcesEnabled && Boolean(clusterContext) && sourceReady,
		staleTime: 30_000,
		retry: false,
	}));
	const fetchKeys = $derived(buildFetchKeys(selectedNamespaces, selectedKinds));
	const resourceQueryKey = $derived(
		queryKeys.resources(clusterContext, fetchKeys, kubeconfigSourceKey),
	);
	const topologyNamespaces = $derived([...new Set(selectedNamespaces)].sort());
	const topologyCustomResourceKey = $derived(
		selectedKinds
			.filter((kind) => typeof kind !== "string")
			.map(kindSelectionKey)
			.sort()
			.join(","),
	);
	const topologyBaseQueryKey = $derived(
		queryKeys.resourceTopology(
			clusterContext,
			topologyNamespaces,
			topologyMode,
			kubeconfigSourceKey,
		),
	);
	const topologyQueryKey = $derived([...topologyBaseQueryKey, topologyCustomResourceKey] as const);
	const topologyFitViewKey = $derived(JSON.stringify(topologyBaseQueryKey));
	const metricsQueryKey = $derived(
		queryKeys.resourceMetrics(clusterContext, topologyNamespaces, kubeconfigSourceKey),
	);
	const resourceCancelScope = $derived(createCancelScope("resources", resourceQueryKey));
	const topologyCancelScope = $derived(createCancelScope("resource-topology", topologyQueryKey));
	const metricsCancelScope = $derived(createCancelScope("resource-metrics", metricsQueryKey));
	const pendingCancelTimers = new Map<string, ReturnType<typeof setTimeout>>();
	const resourcesQuery = createQuery<ResourceSummary[]>(() => ({
		queryKey: resourceQueryKey,
		queryFn: () => fetchResourcePage(clusterContext, fetchKeys, kubeconfigSourceKey, resourceCancelScope),
		enabled: Boolean(clusterContext && fetchKeys.length > 0 && sourceReady),
		placeholderData: (previousData) => previousData,
		staleTime: 30_000,
	}));
	$effect(() => {
		if (!customResourcesEnabled) {
			const nativeKinds = selectedKinds.filter((kind) => typeof kind === "string");
			if (nativeKinds.length !== selectedKinds.length) selectedKinds = nativeKinds;
			appliedAvailableKindsKey = availableKindsKey;
			return;
		}
		if (
			appliedScopeKey === "" ||
			appliedAvailableKindsKey === availableKindsKey ||
			!resourcesQuery.isSuccess ||
			resourcesQuery.isPlaceholderData
		) return;
		const previous = new Set(appliedAvailableKindsKey.split(",").filter(Boolean));
		const selected = new Set(selectedKinds.map(kindSelectionKey));
		const additions = availableKinds.filter((kind) => {
			const key = kindSelectionKey(kind);
			return !previous.has(key) && !selected.has(key);
		});
		appliedAvailableKindsKey = availableKindsKey;
		if (additions.length > 0) selectedKinds = [...selectedKinds, ...additions];
	});
	const topologyQuery = createQuery<ResourceTopology>(() => ({
		queryKey: topologyQueryKey,
		queryFn: () =>
			withForegroundLoad("resource-topology", () =>
				listResourceTopology(
					client,
					clusterContext,
					topologyNamespaces,
					topologyMode,
					kubeconfigSourceKey,
					createCancellableRequest(topologyCancelScope, "topology"),
				).catch((error: unknown) => {
					if (isAppError(error) && error.kind === "cancelled") {
						diagnosticLog("resources.topology.cancel", {
							namespaces: topologyNamespaces.length,
						});
					}
					throw error;
				}),
			),
		enabled: Boolean(clusterContext && mapPanelOpen),
		placeholderData: (previousData) => previousData,
		staleTime: 30_000,
		retry: false,
	}));

	function cancelPendingBackendScope(cancelScope: string) {
		const timer = pendingCancelTimers.get(cancelScope);
		if (!timer) return;
		clearTimeout(timer);
		pendingCancelTimers.delete(cancelScope);
	}

	function scheduleBackendScopeCancel(
		cancelScope: string,
		queryKey: readonly unknown[],
		event: string,
	) {
		cancelPendingBackendScope(cancelScope);
		const timer = setTimeout(() => {
			pendingCancelTimers.delete(cancelScope);
			void queryClient.cancelQueries({ queryKey, exact: true });
			void cancelBackendRequests(client, cancelScope)
				.then((result) => {
					if (result.cancelled > 0) diagnosticLog(event, { cancelled: result.cancelled });
				})
				.catch((error: unknown) => {
					diagnosticLog(`${event}.error`, {
						error: error instanceof Error ? error.message : String(error),
					});
				});
		}, 0);
		pendingCancelTimers.set(cancelScope, timer);
	}

	$effect(() => {
		const currentResourceCancelScope = resourceCancelScope;
		const currentResourceQueryKey = resourceQueryKey;
		const currentTopologyCancelScope = topologyCancelScope;
		const currentTopologyQueryKey = topologyQueryKey;
		const currentMetricsCancelScope = metricsCancelScope;
		const currentMetricsQueryKey = metricsQueryKey;
		cancelPendingBackendScope(currentResourceCancelScope);
		cancelPendingBackendScope(currentTopologyCancelScope);
		cancelPendingBackendScope(currentMetricsCancelScope);
		return () => {
			scheduleBackendScopeCancel(
				currentResourceCancelScope,
				currentResourceQueryKey,
				"resources.scope.cancel",
			);
			scheduleBackendScopeCancel(
				currentTopologyCancelScope,
				currentTopologyQueryKey,
				"resources.topology.cancel",
			);
			scheduleBackendScopeCancel(
				currentMetricsCancelScope,
				currentMetricsQueryKey,
				"resources.metrics.cancel",
			);
		};
	});
	$effect(() => {
		const metricsScopeKey = JSON.stringify(metricsQueryKey);
		if (appliedMetricsScopeKey === metricsScopeKey) return;
		appliedMetricsScopeKey = metricsScopeKey;
		metricsQueryReady = false;
	});
	$effect(() => {
		const rowCount = resourcesQuery.data?.length ?? 0;
		if (
			metricsQueryReady ||
			!clusterContext ||
			!resourcesQuery.isSuccess ||
			resourcesQuery.isPlaceholderData ||
			(mapPanelOpen && topologyQuery.isPending) ||
			typeof window === "undefined"
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
		queryKey: metricsQueryKey,
		queryFn: () =>
			listResourceMetrics(
				client,
				clusterContext,
				topologyNamespaces,
				kubeconfigSourceKey,
				createCancellableRequest(metricsCancelScope, "metrics"),
			).catch((error: unknown) => {
				if (isAppError(error) && error.kind === "cancelled") {
					diagnosticLog("resources.metrics.cancel", {
						namespaces: topologyNamespaces.length,
					});
				}
				throw error;
			}),
		enabled: metricsQueryReady && Boolean(clusterContext),
		retry: false,
		staleTime: 30_000,
	}));

	const namespaceOptions = $derived(namespacesQuery.data ?? []);
	const kindOptions = $derived(
		allKindOptions(customResourcesEnabled ? (resourceKindsQuery.data ?? []) : []),
	);
	const selectedNamespaceSet = $derived(new Set(selectedNamespaces));
	const selectedKindSet = $derived(new Set(selectedKinds.map(kindSelectionKey)));
	const rowsWithMetrics = $derived(
		mergeResourceMetrics(resourcesQuery.data ?? [], metricsQuery.data),
	);
	const topologyWithMetrics = $derived(
		mergeTopologyMetrics(topologyQuery.data, metricsQuery.data),
	);
	const tableModel = $derived(
		buildResourceTableModel(rowsWithMetrics, {
			search,
			gitOpsFilter,
			healthFilter,
			sort: { id: sortColumn, desc: sortDesc },
			pageIndex,
			collapsedGroups,
			selectedResource,
		}),
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
			105 +
			95 +
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
		selectedResource ? resourceSelectionKey(selectedResource) : "",
	);
	const selectedResourceIdentityKey = $derived(
		selectedResource ? resourceIdentityKey(selectedResource) : "",
	);
	const hasExactSelectedResource = $derived(
		Boolean(selectedResourceKey) &&
			tableModel.pageRows.some((row) => resourceSelectionKey(row) === selectedResourceKey),
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
		targetResource ? resourceSelectionKey(targetResource) : "",
	);
	const targetResourceIdentityKey = $derived(
		targetResource ? resourceIdentityKey(targetResource) : "",
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
		sourceQuery.isError
			? sourceQuery.error
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
		const resizeObserver =
			typeof ResizeObserver === "undefined" ? null : new ResizeObserver(measure);
		resizeObserver?.observe(viewport);
		return () => {
			window.cancelAnimationFrame(frame);
			resizeObserver?.disconnect();
		};
	});

	$effect(() => {
		const viewport = tableViewportElement;
		const selectionKey = selectedResourceKey;
		const identityKey = selectedResourceIdentityKey;
		const scrollKey = `${selectionKey}:${identityKey}`;
		if (!selectionKey && !identityKey) {
			appliedSelectionScrollKey = "";
			return;
		}
		if (appliedSelectionScrollKey === scrollKey) return;
		const entries = tableModel.entries;
		if (!tablePanelOpen || !viewport || (!selectionKey && !identityKey) || entries.length === 0) {
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
		const matchedResource = tableModel.displayRows.find(
			(row) => resourceMatchesKeys(row, targetResourceKey, targetResourceIdentityKey, false),
		);
		if (!matchedResource) return;
		const rowIndex = tableModel.displayRows.findIndex(
			(row) => resourceMatchesKeys(row, targetResourceKey, targetResourceIdentityKey, false),
		);
		onResourceSelect(matchedResource, "restore");
		selectedTopologyNodeId = null;
		pageIndex = Math.max(0, Math.floor(rowIndex / PAGE_SIZE));
		appliedTargetResourceKey = targetResourceKey;
	});

	$effect(() => {
		const enabled =
			Boolean(clusterContext) &&
			resourcesQuery.isSuccess &&
			!resourcesQuery.isPlaceholderData &&
			fetchKeys.length > 0 &&
			!resourceError;
		const watchKeyMap = new Map(
			[...watchKeysFromFetchKeys(fetchKeys), ...topologyWatchKeys(topologyNamespaces)].map(
				(key) => [JSON.stringify(key), key],
			),
		);
		const watchKeys = Array.from(watchKeyMap.values());
		if (!enabled || watchKeys.length === 0) {
			realtimeStatus = "idle";
			realtimeMessage = "Realtime idle";
			realtimeError = "";
			return;
		}
		let cancelled = false;
		let streamId: string | null = null;
		let debounce: ReturnType<typeof setTimeout> | null = null;
		const startedAt = performance.now();
		realtimeStatus = "connecting";
		realtimeMessage = "Starting realtime watch";
		realtimeError = "";
		const invalidateSoon = () => {
			if (debounce) clearTimeout(debounce);
			debounce = setTimeout(() => {
				void queryClient.invalidateQueries({ queryKey: resourceQueryKey });
				void queryClient.invalidateQueries({ queryKey: topologyQueryKey });
			}, 250);
		};
		const channel = createStreamChannel((event) => {
			if (cancelled) return;
			if (event.type === "started") {
				streamId = event.streamId;
				realtimeMessage = "Realtime watch starting";
				return;
			}
			if (event.type === "status") {
				realtimeStatus = event.status;
				realtimeMessage = event.message;
				realtimeError = "";
				return;
			}
			if (event.type === "resourceChanged") {
				realtimeStatus = "connected";
				realtimeMessage = `Realtime ${event.action}`;
				realtimeError = "";
				if (shouldDropWarmupWatchEvent(event.action, performance.now() - startedAt)) return;
				invalidateSoon();
				return;
			}
			if (event.type === "error") {
				realtimeStatus = "error";
				realtimeMessage = "Realtime watch error";
				realtimeError = event.message;
				return;
			}
			if (event.type === "stopped") {
				realtimeStatus = "stopped";
				realtimeMessage = "Realtime stopped";
				realtimeError = "";
			}
		});
		void startResourceWatch(
			client,
			clusterContext,
			watchKeys,
			channel,
			kubeconfigSourceKey,
		)
			.then((id) => {
				if (cancelled) {
					void stopStream(client, id);
					return;
				}
				streamId = id;
			})
			.catch((error: unknown) => {
				if (cancelled) return;
				realtimeStatus = "error";
				realtimeMessage = "Realtime watch failed";
				realtimeError = error instanceof Error ? error.message : String(error);
			});
		return () => {
			cancelled = true;
			if (debounce) clearTimeout(debounce);
			if (streamId) void stopStream(client, streamId);
			closeStreamChannel(channel);
		};
	});

	function toggleNamespace(namespace: string, checked: boolean) {
		selectedNamespaces = checked
			? Array.from(new Set([...selectedNamespaces, namespace])).sort()
			: selectedNamespaces.filter((item) => item !== namespace);
		pageIndex = 0;
		onResourceClose();
		selectedTopologyNodeId = null;
	}

	function toggleKind(kind: ResourceKindSelection, checked: boolean) {
		const key = kindSelectionKey(kind);
		selectedKinds = checked
			? [...selectedKinds, kind]
			: selectedKinds.filter((item) => kindSelectionKey(item) !== key);
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

	function clearFilters() {
		search = "";
		gitOpsFilter = "";
		healthFilter = "all";
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

	function resourceMatchesKeys(
		resource: ResourceSummary,
		selectionKey: string,
		identityKey: string,
		exactMatchExists: boolean,
	): boolean {
		return (
			resourceSelectionKey(resource) === selectionKey ||
			(!exactMatchExists && resourceIdentityKey(resource) === identityKey)
		);
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

	function compactNamespaceList(values: string[]): string {
		if (values.length === 0) return "All namespaces";
		if (values.length <= 3) return values.join(", ");
		return `${values.slice(0, 3).join(", ")} +${values.length - 3}`;
	}

	function statusBadgeVariant(tone: ChipVariant) {
		return STATUS_BADGE_STYLES[tone].variant;
	}

	function statusBadgeClass(tone: ChipVariant) {
		return `rounded-full px-2 py-0 text-[0.6875rem] shadow-none ${STATUS_BADGE_STYLES[tone].className}`;
	}
</script>

<div class="flex h-full min-h-0 min-w-0 flex-col gap-3">
	{#if gitOpsFocusApplication}
		{@const app = gitOpsFocusApplication}
		{@const destination = app.destinationNamespace ?? app.destinationServer}
		<section class="rounded-lg border border-sidebar-border bg-surface-1 px-3 py-2.5 shadow-sm">
			<div class="flex flex-wrap items-start justify-between gap-3">
				<div class="min-w-0">
					<div class="flex min-w-0 items-center gap-2">
						<GitBranch class="size-4 shrink-0 text-[var(--resource-argo)]" />
						<h2 class="truncate text-base font-semibold">{app.name}</h2>
					</div>
					<div class="mt-2 flex flex-wrap gap-1.5">
						{#if app.healthStatus}
							{@const healthTone = healthStatusVariant(app.healthStatus)}
							<Badge variant={statusBadgeVariant(healthTone)} class={statusBadgeClass(healthTone)}>
								{app.healthStatus}
							</Badge>
						{/if}
						{#if app.syncStatus}
							{@const syncTone = syncStatusVariant(app.syncStatus)}
							<Badge variant={statusBadgeVariant(syncTone)} class={statusBadgeClass(syncTone)}>
								{app.syncStatus}
							</Badge>
						{/if}
					</div>
				</div>
				<div class="text-right text-sm">
					<div class="font-semibold text-foreground">{app.trackedResourceCount ?? "-"}</div>
					<div class="text-xs text-muted-foreground">tracked resources</div>
				</div>
			</div>
			<div class="mt-2.5 grid gap-x-4 gap-y-2 sm:grid-cols-2 lg:grid-cols-4 2xl:grid-cols-7">
				{@render SummaryField("Project", app.project)}
				{@render SummaryField("Repo", app.sourceRepo, app.sourceRepo ?? undefined)}
				{@render SummaryField("Revision", app.sourceRevision)}
				{@render SummaryField("Destination", destination)}
				{@render SummaryField("App namespace", app.namespace)}
				{@render SummaryField(
					"Resource namespaces",
					compactNamespaceList(app.resourceNamespaces),
					app.resourceNamespaces.length > 0 ? app.resourceNamespaces.join(", ") : undefined,
				)}
				{@render SummaryField("Created", app.age, app.createdAt)}
			</div>
		</section>
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
	{:else if resourceError}
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
			{metricsMessage}
			{customResourcesStatus}
			{realtimeStatus}
			{realtimeMessage}
			onAllNamespacesSelect={() => {
				selectedNamespaces = [];
				pageIndex = 0;
				onResourceClose();
				selectedTopologyNodeId = null;
			}}
			onNamespaceToggle={toggleNamespace}
			onKindToggle={toggleKind}
			onHealthSelect={selectHealth}
			onGitOpsFilterChange={setGitOpsFilter}
			onSearchInput={() => (pageIndex = 0)}
			onClearFilters={clearFilters}
		/>
		{#if realtimeError}
			<FriendlyError
				mode="compact"
				error={realtimeError}
				context={{
					operation: "resourcesLoad",
					fallbackTitle: "Realtime watch failed",
					partial: true,
				}}
			/>
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
					<OwnershipMap
						topology={topologyWithMetrics}
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
								<col style="width: 105px;" />
								<col style="width: 95px;" />
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
									<TableHead>{@render SortButton("namespace", "Namespace")}</TableHead>
									<TableHead>{@render SortButton("kind", "Kind")}</TableHead>
									<TableHead>{@render SortButton("status", "Status")}</TableHead>
									{#if tableModel.columnVisibility.ready}<TableHead>{@render SortButton("ready", "Ready")}</TableHead>{/if}
									{#if tableModel.columnVisibility.restarts}<TableHead>{@render SortButton("restarts", "Restarts")}</TableHead>{/if}
									{#if tableModel.columnVisibility.cpu}<TableHead>{@render SortButton("cpu", "CPU")}</TableHead>{/if}
									{#if tableModel.columnVisibility.memory}<TableHead>{@render SortButton("memory", "Memory")}</TableHead>{/if}
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
											{@const rowSelected = resourceMatchesKeys(row, selectedResourceKey, selectedResourceIdentityKey, hasExactSelectedResource)}
											<TableRow
												data-resource-selected={rowSelected ? "true" : undefined}
												class={cnfast(ROW_CLASS, rowSelected && SELECTED_ROW_CLASS)}
												tabindex="0"
												role="button"
												aria-pressed={rowSelected}
												onclick={() => selectResource(row)}
												onkeydown={(event: KeyboardEvent) => {
													if (event.key === "Enter" || event.key === " ") {
														event.preventDefault();
														selectResource(row);
													}
												}}
											>
												<TableCell class="font-medium">
													<span class="block min-w-0 truncate" title={row.name}>{row.name}</span>
												</TableCell>
												<TableCell>{row.namespace ?? EMPTY_CELL}</TableCell>
												<TableCell>{row.kind}</TableCell>
												<TableCell>
													{#if row.status}
														{@const statusTone = resourceStatusTone(row.status)}
														<Badge variant={statusBadgeVariant(statusTone)} class={statusBadgeClass(statusTone)}>
															{row.status}
														</Badge>
													{:else}
														{EMPTY_CELL}
													{/if}
												</TableCell>
												{#if tableModel.columnVisibility.ready}
													<TableCell>
														{@const ready = resourceReadyChip(row)}
														{#if ready}
															<Badge variant={statusBadgeVariant(ready.tone)} class={statusBadgeClass(ready.tone)}>
																{ready.value}
															</Badge>
														{:else}
															{row.ready ?? EMPTY_CELL}
														{/if}
													</TableCell>
												{/if}
												{#if tableModel.columnVisibility.restarts}
													<TableCell>
														{#if row.restarts === undefined || row.restarts === null}
															<span class="flex justify-center">{EMPTY_CELL}</span>
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
													<TableCell>{formatCpuMillicores(row.metrics?.cpuMillicores)}</TableCell>
												{/if}
												{#if tableModel.columnVisibility.memory}
													<TableCell>{formatMemoryBytes(row.metrics?.memoryBytes)}</TableCell>
												{/if}
												{#if tableModel.columnVisibility.gitOps}
													<TableCell>
														<span class="block min-w-0 truncate">
															{row.gitOpsOwner?.name ?? row.argoApp ?? row.helmRelease ?? EMPTY_CELL}
														</span>
													</TableCell>
												{/if}
												<TableCell>{row.age}</TableCell>
												<TableCell>
													<Button
														type="button"
														variant="ghost"
														size="icon"
														class="size-7"
														aria-label={`${pinnedResourceKeySet.has(resourceSelectionKey(row)) ? "Unpin" : "Pin"} ${row.kind} ${row.name}`}
														onclick={(event: MouseEvent) => {
															event.stopPropagation();
															onResourcePinToggle(row);
														}}
														onkeydown={(event: KeyboardEvent) => event.stopPropagation()}
													>
														<Pin
															class={pinnedResourceKeySet.has(resourceSelectionKey(row)) ? "fill-current" : ""}
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

{#snippet SummaryField(label: string, value: string | number | null | undefined, title: string | undefined = undefined)}
	<div class="min-w-0">
		<div class="text-[11px] font-medium uppercase text-muted-foreground">{label}</div>
		<div class="min-w-0 truncate text-sm text-foreground" {title}>{value ?? "-"}</div>
	</div>
{/snippet}

{#snippet SortButton(column: ResourceSortColumn, label: string)}
	<Button
		type="button"
		variant="ghost"
		size="xs"
		class="h-auto gap-1 border-0 bg-transparent p-0 text-left text-inherit hover:bg-transparent"
		onclick={() => toggleSort(column)}
		aria-label={`Sort by ${label}`}
	>
		{label}
		{#if sortColumn === column && sortDesc}<ArrowDown class="size-3" aria-hidden="true" />{:else if sortColumn === column}<ArrowUp class="size-3" aria-hidden="true" />{:else}<ChevronsUpDown class="size-3" aria-hidden="true" />{/if}
	</Button>
{/snippet}
