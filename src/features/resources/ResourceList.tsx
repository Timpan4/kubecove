import {
	memo,
	useDeferredValue,
	useEffect,
	useMemo,
	useRef,
	useState,
} from "react";
import { keepPreviousData, useQuery } from "@tanstack/react-query";
import type { SortingState } from "@tanstack/react-table";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { SearchX } from "lucide-react";
import {
	Empty,
	EmptyDescription,
	EmptyHeader,
	EmptyMedia,
	EmptyTitle,
} from "@/components/ui/empty";
import { Skeleton } from "@/components/ui/skeleton";
import { Spinner } from "@/components/ui/spinner";
import {
	createCancellableRequest,
	createCancelScope,
	useCancelBackendScopes,
} from "@/lib/cancellable-loads";
import { diagnosticLog } from "@/lib/diagnostics";
import { withForegroundLoad } from "@/lib/foreground-loading";
import { queryKeys } from "@/lib/queryKeys";
import { useSettingsState } from "@/lib/settings";
import {
	createTauriClient,
	isAppError,
	listResourceTopology,
} from "@/lib/tauri";
import {
	describeMetricsAvailability,
	mergeResourceMetrics,
	mergeTopologyMetrics,
} from "@/lib/resource-metrics";
import type {
	ResourceKindSelection,
	ResourceSummary,
	TopologyMode,
	TopologyNode,
} from "@/lib/types";
import { cnfast } from "@/lib/utils";
import { PAGE_SIZE } from "./constants";
import { pageGitOpsGroupCounts, pageTypeGroupCounts } from "./grouping";
import {
	buildFetchKeys,
	buildResourceHealthSummary,
	buildResourceSearchIndex,
	describeResourceScope,
	filterResourcesByHealth,
	filterResourceSearchIndex,
	formatResourceGroupLabel,
	hasResourceListGitOpsOwner,
	resourceGroupCollapseKey,
	resourceGroupKindRank,
	resourceIdentityKey,
	resourceSelectionKey,
	type HealthFilter,
	resourceTypeGroupCollapseKey,
	sortedRows,
	topologyWatchKeys,
	uniqueGitOpsFilters,
	watchKeysFromFetchKeys,
} from "./helpers";
import {
	ActiveHealthFilterBanner,
	ResourceHealthStrip,
} from "./health";
import { fetchResourcePage } from "./query";
import {
	ResourceGitOpsFocusSummary,
	type ResourceGitOpsFocus,
} from "./ResourceGitOpsFocusSummary";
import { ResourceScopePills } from "./scope-filters";
import { ResourceTableLayoutBoundary } from "./ResourceTableLayoutBoundary";
import { ResourceToolbar } from "./toolbar";
import { useDeferredResourceMetricsQuery } from "./useDeferredResourceMetrics";
import { useResourceWatch } from "./useResourceWatch";

interface ResourceListProps {
	clusterContext: string;
	selectedNamespaces: string[];
	selectedKinds: ResourceKindSelection[];
	selectedArgoAppFilter: string;
	gitOpsFocus?: ResourceGitOpsFocus | null;
	selectedResource: ResourceSummary | null;
	initialHealthFilter?: HealthFilter;
	initialSearch?: string;
	onArgoAppFilterChange: (app: string) => void;
	onNamespacesChange: (namespaces: string[]) => void;
	onKindsChange: (kinds: ResourceKindSelection[]) => void;
	onResourceSelect: (resource: ResourceSummary) => void;
}

function ResourceListComponent({
	clusterContext,
	selectedNamespaces,
	selectedKinds,
	selectedArgoAppFilter,
	gitOpsFocus = null,
	selectedResource,
	initialHealthFilter = "all",
	initialSearch = "",
	onArgoAppFilterChange,
	onNamespacesChange,
	onKindsChange,
	onResourceSelect,
}: ResourceListProps) {
	const [pageIndex, setPageIndex] = useState(0);
	const [sorting, setSorting] = useState<SortingState>([]);
	const [search, setSearch] = useState(initialSearch);
	const [collapsedGroups, setCollapsedGroups] = useState<Set<string>>(
		() => new Set(),
	);
	const [healthFilter, setHealthFilter] =
		useState<HealthFilter>(initialHealthFilter);
	const [selectedTopologyNodeId, setSelectedTopologyNodeId] = useState<
		string | null
	>(null);
	const [topologyMode, setTopologyMode] = useState<TopologyMode>("ownership");
	const showOwnershipMapByDefault = useSettingsState(
		(state) => state.showOwnershipMapByDefault,
	);
	const showFullTopologyOnSelection = useSettingsState(
		(state) => state.showFullTopologyOnSelection,
	);
	const [mapPanelOpen, setMapPanelOpen] = useState(showOwnershipMapByDefault);
	const client = useMemo(() => createTauriClient(), []);
	const kubeconfigEnvVar = useSettingsState((state) => state.kubeconfigSourceKey);
	const renderCountRef = useRef(0);

	const topologyNamespaceKey = useMemo(
		() =>
			Array.from(new Set(selectedNamespaces))
				.toSorted((a, b) => a.localeCompare(b))
				.join(","),
		[selectedNamespaces],
	);
	const topologyNamespaces = useMemo(
		() => (topologyNamespaceKey ? topologyNamespaceKey.split(",") : []),
		[topologyNamespaceKey],
	);
	const fetchKeys = useMemo(
		() => buildFetchKeys(selectedNamespaces, selectedKinds),
		[selectedNamespaces, selectedKinds],
	);
	const queryKey = useMemo(
		() => queryKeys.resources(clusterContext, fetchKeys, kubeconfigEnvVar),
		[clusterContext, fetchKeys, kubeconfigEnvVar],
	);
	const topologyQueryKey = useMemo(
		() =>
			queryKeys.resourceTopology(
				clusterContext,
				topologyNamespaces,
				topologyMode,
				kubeconfigEnvVar,
			),
		[clusterContext, topologyMode, topologyNamespaces, kubeconfigEnvVar],
	);
	const resourceCancelScope = useMemo(
		() => createCancelScope("resources", queryKey),
		[queryKey],
	);
	const topologyCancelScope = useMemo(
		() => createCancelScope("resource-topology", topologyQueryKey),
		[topologyQueryKey],
	);
	const topologyFitViewKey = useMemo(
		() => JSON.stringify(topologyQueryKey),
		[topologyQueryKey],
	);
	const cancelEntries = useMemo(
		() => [
			{
				cancelScope: resourceCancelScope,
				queryKey,
				event: "resources.scope.cancel",
			},
			{
				cancelScope: topologyCancelScope,
				queryKey: topologyQueryKey,
				event: "resources.topology.cancel",
			},
		],
		[queryKey, resourceCancelScope, topologyCancelScope, topologyQueryKey],
	);
	useCancelBackendScopes(client, cancelEntries);

	const { data, isPending, isError, error, isPlaceholderData } = useQuery({
		queryKey,
		queryFn: () =>
			fetchResourcePage(
				clusterContext,
				fetchKeys,
				kubeconfigEnvVar,
				resourceCancelScope,
			),
		enabled: fetchKeys.length > 0,
		staleTime: 30_000,
		placeholderData: keepPreviousData,
	});
	const {
		data: topologyData,
		isPending: topologyPending,
		isError: topologyError,
		error: topologyErr,
	} = useQuery({
		queryKey: topologyQueryKey,
		queryFn: () =>
			withForegroundLoad("resource-topology", () =>
				listResourceTopology(
					client,
					clusterContext,
					topologyNamespaces,
					topologyMode,
					kubeconfigEnvVar,
					createCancellableRequest(topologyCancelScope, "topology"),
				).catch((error) => {
					if (isAppError(error) && error.kind === "cancelled") {
						diagnosticLog("resources.topology.cancel", {
							namespaces: topologyNamespaces.length,
						});
					}
					throw error;
				}),
			),
		enabled: Boolean(clusterContext && mapPanelOpen),
		staleTime: 30_000,
		placeholderData: keepPreviousData,
	});
	const { data: metricsData, isError: metricsError } =
		useDeferredResourceMetricsQuery({
			client,
			clusterContext,
			namespaces: topologyNamespaces,
			kubeconfigEnvVar,
			resourceRowsReady: Boolean(data && !isPending && !isPlaceholderData),
			resourceRowCount: data?.length ?? 0,
			waitForTopology: mapPanelOpen,
			topologyPending,
	});
	const tableWatchKeys = useMemo(
		() => watchKeysFromFetchKeys(fetchKeys),
		[fetchKeys],
	);
	const topologyResourceWatchKeys = useMemo(
		() => (mapPanelOpen ? topologyWatchKeys(topologyNamespaces) : []),
		[mapPanelOpen, topologyNamespaces],
	);
	const watchSubscriptions = useMemo(
		() => [
			{ keys: tableWatchKeys, queryKeys: [queryKey] },
			{ keys: topologyResourceWatchKeys, queryKeys: [topologyQueryKey] },
		],
		[queryKey, tableWatchKeys, topologyQueryKey, topologyResourceWatchKeys],
	);
	const realtime = useResourceWatch({
		client,
		clusterContext,
		kubeconfigEnvVar,
		subscriptions: watchSubscriptions,
		enabled:
			watchSubscriptions.some((subscription) => subscription.keys.length > 0) &&
			Boolean(data && !isPending && !isPlaceholderData) &&
			!isError,
	});

	const dataWithMetrics = useMemo(
		() => mergeResourceMetrics(data ?? [], metricsData),
		[data, metricsData],
	);
	const topologyWithMetrics = useMemo(
		() =>
			mapPanelOpen
				? mergeTopologyMetrics(topologyData, metricsData)
				: undefined,
		[mapPanelOpen, topologyData, metricsData],
	);
	const metricsAvailabilityMessage = describeMetricsAvailability(
		metricsData?.availability,
	);
	const gitOpsFilters = useMemo(
		() => uniqueGitOpsFilters(dataWithMetrics),
		[dataWithMetrics],
	);
	const resourceSearchIndex = useMemo(
		() => buildResourceSearchIndex(dataWithMetrics),
		[dataWithMetrics],
	);
	const scopedData = useMemo(
		() => filterResourceSearchIndex(resourceSearchIndex, search, selectedArgoAppFilter),
		[resourceSearchIndex, search, selectedArgoAppFilter],
	);
	const filteredData = useMemo(
		() => filterResourcesByHealth(scopedData, healthFilter),
		[scopedData, healthFilter],
	);
	const sortedData = useMemo(
		() => sortedRows(filteredData, sorting),
		[filteredData, sorting],
	);
	const groupedByGitOps = useMemo(
		() => filteredData.some(hasResourceListGitOpsOwner),
		[filteredData],
	);
	const displayData = useMemo(() => {
		if (!groupedByGitOps) return sortedData;
		return sortedData.toSorted((a, b) => {
			const groupCompare = formatResourceGroupLabel(a).localeCompare(
				formatResourceGroupLabel(b),
			);
			if (groupCompare !== 0) return groupCompare;
			const kindRankCompare =
				resourceGroupKindRank(a.kind) - resourceGroupKindRank(b.kind);
			if (kindRankCompare !== 0) return kindRankCompare;
			const kindCompare = a.kind.localeCompare(b.kind);
			if (kindCompare !== 0) return kindCompare;
			return a.name.localeCompare(b.name);
		});
	}, [groupedByGitOps, sortedData]);
	const healthSummary = useMemo(
		() => buildResourceHealthSummary(scopedData),
		[scopedData],
	);
	const scopePills = useMemo(
		() =>
			describeResourceScope(
				selectedNamespaces,
				selectedKinds,
				selectedArgoAppFilter,
			),
		[selectedNamespaces, selectedKinds, selectedArgoAppFilter],
	);
	const resettableScopeKey = useMemo(
		() => JSON.stringify(queryKey),
		[queryKey],
	);
	const [previousResetState, setPreviousResetState] = useState(() => ({
		healthFilter: initialHealthFilter,
		scopeKey: resettableScopeKey,
		search: initialSearch,
	}));

	if (
		previousResetState.scopeKey !== resettableScopeKey ||
		previousResetState.search !== initialSearch ||
		previousResetState.healthFilter !== initialHealthFilter
	) {
		setPreviousResetState({
			healthFilter: initialHealthFilter,
			scopeKey: resettableScopeKey,
			search: initialSearch,
		});
		if (previousResetState.scopeKey !== resettableScopeKey) {
			setPageIndex(0);
			setCollapsedGroups(new Set());
			setSelectedTopologyNodeId(null);
		}
		if (previousResetState.search !== initialSearch) {
			setSearch(initialSearch);
		}
		if (previousResetState.healthFilter !== initialHealthFilter) {
			setHealthFilter(initialHealthFilter);
		}
	}

	const totalRows = displayData.length;
	const pageCount = Math.max(1, Math.ceil(totalRows / PAGE_SIZE));
	const safePageIndex = Math.min(pageIndex, Math.max(0, pageCount - 1));
	const startRow = safePageIndex * PAGE_SIZE;
	const endRow = startRow + PAGE_SIZE;
	const pageRows = useMemo(
		() => displayData.slice(startRow, endRow),
		[displayData, startRow, endRow],
	);
	const pageGroups = useMemo(
		() => pageGitOpsGroupCounts(pageRows, groupedByGitOps),
		[groupedByGitOps, pageRows],
	);
	const pageTypeGroups = useMemo(
		() => pageTypeGroupCounts(pageRows, groupedByGitOps),
		[groupedByGitOps, pageRows],
	);

	// Hide columns that would render "—" for every visible row so the name
	// column keeps its width for what actually matters.
	const columnVisibility = useMemo(
		() => ({
			ready: pageRows.some((row) => Boolean(row.ready)),
			restarts: pageRows.some((row) => row.restarts !== undefined),
			cpu: pageRows.some((row) => row.metrics?.cpuMillicores !== undefined),
			memory: pageRows.some((row) => row.metrics?.memoryBytes !== undefined),
			ownerRef: false,
			"argo-helm": pageRows.some(
				(row) =>
					Boolean(row.gitOpsOwner) ||
					Boolean(row.argoApp) ||
					Boolean(row.helmRelease),
			),
		}),
		[pageRows],
	);
	const tableRenderKey = useMemo(() => {
		const visibilityKey = Object.entries(columnVisibility)
			.toSorted(([left], [right]) => left.localeCompare(right))
			.map(([columnId, visible]) => `${columnId}:${visible ? "1" : "0"}`)
			.join(",");
		const sortingKey = sorting
			.map((sort) => `${sort.id}:${sort.desc ? "desc" : "asc"}`)
			.join(",");
		return `${visibilityKey}|${sortingKey}`;
	}, [columnVisibility, sorting]);

	const externalSelectedResourceKey = selectedResource
		? resourceSelectionKey(selectedResource)
		: null;
	const externalSelectedResourceIdentityKey = selectedResource
		? resourceIdentityKey(selectedResource)
		: null;
	const activeSelectedResourceKey = externalSelectedResourceKey;
	const activeSelectedResourceIdentityKey = externalSelectedResourceIdentityKey;

	const effectiveCollapsedGroups = useMemo(() => {
		if (!activeSelectedResourceKey && !activeSelectedResourceIdentityKey) {
			return collapsedGroups;
		}
		const selectedRow = displayData.find(
			(resource) =>
				resourceSelectionKey(resource) === activeSelectedResourceKey ||
				resourceIdentityKey(resource) === activeSelectedResourceIdentityKey,
		);
		if (!selectedRow) return collapsedGroups;
		const groupCollapseKey = resourceGroupCollapseKey(selectedRow);
		const typeCollapseKey = resourceTypeGroupCollapseKey(selectedRow);
		if (
			!collapsedGroups.has(groupCollapseKey) &&
			!collapsedGroups.has(typeCollapseKey)
		) {
			return collapsedGroups;
		}
		const next = new Set(collapsedGroups);
		next.delete(groupCollapseKey);
		next.delete(typeCollapseKey);
		return next;
	}, [
		activeSelectedResourceIdentityKey,
		activeSelectedResourceKey,
		collapsedGroups,
		displayData,
	]);

	useEffect(() => {
		renderCountRef.current += 1;
		diagnosticLog("resources.render", {
			render: renderCountRef.current,
			pending: isPending,
			placeholder: isPlaceholderData,
			error: isError,
			rawRows: data?.length ?? 0,
			pageRows: pageRows.length,
			fetches: fetchKeys.length,
			selected: activeSelectedResourceKey ?? "",
		});
	});

	const clearFilters = () => {
		setSearch("");
		setHealthFilter("all");
		onArgoAppFilterChange("");
		setPageIndex(0);
	};
	const toggleGroup = (key: string) => {
		setCollapsedGroups((current) => {
			const next = new Set(current);
			if (next.has(key)) {
				next.delete(key);
			} else {
				next.add(key);
			}
			return next;
		});
	};
	const syncedTopologyNodeId = useMemo(() => {
		const topologyNodes = topologyWithMetrics?.nodes;
		if (!topologyNodes) return selectedTopologyNodeId;
		if (
			selectedTopologyNodeId &&
			!topologyNodes.some((node) => node.id === selectedTopologyNodeId)
		) {
			return null;
		}
		const selectedFromTable =
			topologyNodes.find(
				(node) => resourceSelectionKey(node.summary) === activeSelectedResourceKey,
			) ??
			topologyNodes.find(
				(node) =>
					resourceIdentityKey(node.summary) === activeSelectedResourceIdentityKey,
			);
		return selectedFromTable?.id ?? selectedTopologyNodeId;
	}, [
		activeSelectedResourceIdentityKey,
		activeSelectedResourceKey,
		selectedTopologyNodeId,
		topologyWithMetrics,
	]);
	const deferredSyncedTopologyNodeId = useDeferredValue(syncedTopologyNodeId);
	const handleResourceSelect = (resource: ResourceSummary) => {
		setSelectedTopologyNodeId(null);
		onResourceSelect(resource);
	};
	const handleTopologyNodeSelect = (
		node: TopologyNode,
		resource: ResourceSummary | null,
	) => {
		setSelectedTopologyNodeId(node.id);
		if (!resource) return;
		onResourceSelect(resource);
	};
	const handleMapPanelOpenChange = (open: boolean) => {
		setMapPanelOpen(open);
		if (!open) setSelectedTopologyNodeId(null);
	};

	if (isPending && !isPlaceholderData) {
		return (
			<div className="p-8 text-center text-sm text-muted-foreground">
				<div className="mb-4 flex flex-col gap-px">
					{Array.from({ length: 8 }).map((_, i) => (
						<div key={i} className="flex gap-3 border-b py-2">
							<Skeleton className="h-3.5 w-[180px]" />
							<Skeleton className="h-3.5 w-[100px]" />
							<Skeleton className="h-3.5 w-20" />
							<Skeleton className="h-3.5 w-[70px]" />
						</div>
					))}
				</div>
				<span className="inline-flex items-center gap-2">
					<Spinner className="size-4" />
						Loading resources…
				</span>
			</div>
		);
	}

	if (isError) {
		return (
			<div className="p-4">
				<Alert variant="destructive">
					<AlertTitle>Failed to load resources</AlertTitle>
					<AlertDescription>
						{error instanceof Error ? error.message : "Failed to load resources"}
					</AlertDescription>
				</Alert>
			</div>
		);
	}

	if (!data || data.length === 0) {
		// Keep the scope pills mounted so the empty state is fixable in place
		// and the layout doesn't jump when results disappear.
		return (
			<div className="flex h-full min-h-0 min-w-0 flex-col gap-3">
				{gitOpsFocus && <ResourceGitOpsFocusSummary focus={gitOpsFocus} />}
				<ResourceScopePills
					pills={scopePills}
					clusterContext={clusterContext}
					selectedNamespaces={selectedNamespaces}
					selectedKinds={selectedKinds}
					onNamespaceChange={onNamespacesChange}
					onKindChange={onKindsChange}
				/>
				<Empty className="min-h-64 flex-1 border-0">
					<EmptyHeader>
						<EmptyMedia variant="icon">
							<SearchX />
						</EmptyMedia>
						<EmptyTitle>No resources in this scope</EmptyTitle>
						<EmptyDescription>
							Nothing matched the current namespace and kind selection on{" "}
							{clusterContext}. Adjust the scope filters above to widen the
							search.
						</EmptyDescription>
					</EmptyHeader>
				</Empty>
			</div>
		);
	}

	const showingStaleRows = isPlaceholderData;

	return (
		<div className="flex h-full min-h-0 min-w-0 flex-col gap-3">
			{gitOpsFocus && <ResourceGitOpsFocusSummary focus={gitOpsFocus} />}
			<ResourceScopePills
				pills={scopePills}
				clusterContext={clusterContext}
				selectedNamespaces={selectedNamespaces}
				selectedKinds={selectedKinds}
				onNamespaceChange={onNamespacesChange}
				onKindChange={onKindsChange}
			/>
			<ResourceHealthStrip
				summary={healthSummary}
				activeFilter={healthFilter}
				onFilterChange={(filter) => {
					setHealthFilter(filter);
					setPageIndex(0);
				}}
			/>
			{(metricsAvailabilityMessage || metricsError) && (
				<Alert variant="default" className="py-2">
					<AlertTitle className="text-xs">Resource metrics</AlertTitle>
					<AlertDescription className="text-xs">
						{metricsError
							? "metrics API unavailable"
							: metricsAvailabilityMessage}
					</AlertDescription>
				</Alert>
			)}
			<ActiveHealthFilterBanner
				filter={healthFilter}
				onReset={() => {
					setHealthFilter("all");
					setPageIndex(0);
				}}
			/>
			<ResourceToolbar
				search={search}
				gitOpsFilters={gitOpsFilters}
				selectedArgoAppFilter={selectedArgoAppFilter}
				hasNoFilterResults={filteredData.length === 0}
				onSearchChange={(value) => {
					setSearch(value);
					setPageIndex(0);
				}}
				onArgoAppFilterChange={(value) => {
					onArgoAppFilterChange(value);
					setPageIndex(0);
				}}
				onClearFilters={clearFilters}
			/>
			<div
				className={cnfast(
					"min-h-0 flex-1 transition-opacity",
					showingStaleRows && "opacity-60",
				)}
			>
				<ResourceTableLayoutBoundary
					pageRows={pageRows}
					sorting={sorting}
					columnVisibility={columnVisibility}
					onSortingChange={setSorting}
					topology={topologyWithMetrics}
					topologyLoading={topologyPending}
					topologyError={topologyError}
					topologyErr={topologyErr}
					selectedTopologyNodeId={deferredSyncedTopologyNodeId}
					hasDeferredTopologySelection={Boolean(deferredSyncedTopologyNodeId)}
					topologyFitViewKey={topologyFitViewKey}
					topologyMode={topologyMode}
					onTopologyModeChange={setTopologyMode}
					mapPanelOpen={mapPanelOpen}
					showFullTopologyOnSelection={showFullTopologyOnSelection}
					onMapPanelOpenChange={handleMapPanelOpenChange}
					onTopologyNodeSelect={handleTopologyNodeSelect}
					tableRenderKey={tableRenderKey}
					groupedByGitOps={groupedByGitOps}
					pageGroups={pageGroups}
					pageTypeGroups={pageTypeGroups}
					collapsedGroups={effectiveCollapsedGroups}
					selectedResourceKey={activeSelectedResourceKey}
					selectedResourceIdentityKey={activeSelectedResourceIdentityKey}
					onToggleGroup={toggleGroup}
					onResourceSelect={handleResourceSelect}
					totalRows={totalRows}
					search={search}
					pageIndex={safePageIndex}
					pageCount={pageCount}
					onPageChange={setPageIndex}
					realtime={realtime}
				/>
			</div>
		</div>
	);
}

const MemoizedResourceListComponent = memo(ResourceListComponent);

export function ResourceList(props: ResourceListProps) {
	return <MemoizedResourceListComponent {...props} />;
}
