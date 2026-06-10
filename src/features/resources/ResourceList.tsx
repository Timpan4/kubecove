import {
	memo,
	useDeferredValue,
	useEffect,
	useMemo,
	useRef,
	useState,
} from "react";
import { keepPreviousData, useQuery } from "@tanstack/react-query";
import {
	getCoreRowModel,
	useReactTable,
	type SortingState,
} from "@tanstack/react-table";
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
import { diagnosticLog } from "@/lib/diagnostics";
import { queryKeys } from "@/lib/queryKeys";
import { useSettingsState } from "@/lib/settings";
import {
	createTauriClient,
	listResourceMetrics,
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
import { columns } from "./columns";
import { PAGE_SIZE } from "./constants";
import { pageAppGroupCounts, pageTypeGroupCounts } from "./grouping";
import {
	buildFetchKeys,
	buildResourceHealthSummary,
	buildResourceSearchIndex,
	describeResourceScope,
	filterResourcesByHealth,
	filterResourceSearchIndex,
	formatResourceGroupLabel,
	resourceGroupCollapseKey,
	resourceGroupKindRank,
	resourceIdentityKey,
	resourceTypeGroupCollapseKey,
	resourceSelectionKey,
	type HealthFilter,
	resourceKindFetchKey,
	sortedRows,
	topologyWatchKeys,
	uniqueArgoApps,
	watchKeysFromFetchKeys,
} from "./helpers";
import {
	ActiveHealthFilterBanner,
	ResourceHealthStrip,
} from "./health";
import { fetchResourcePage } from "./query";
import { ResourceMapTableLayout } from "./ResourceMapTableLayout";
import { ResourceScopePills } from "./scope-filters";
import { ResourceToolbar } from "./toolbar";
import { useResourceWatch } from "./useResourceWatch";

interface ResourceListProps {
	clusterContext: string;
	selectedNamespaces: string[];
	selectedKinds: ResourceKindSelection[];
	selectedArgoAppFilter: string;
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
	const [search, setSearch] = useState("");
	const [selectedResourceKey, setSelectedResourceKey] = useState<string | null>(
		null,
	);
	const [selectedResourceIdentityKey, setSelectedResourceIdentityKey] = useState<
		string | null
	>(null);
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
	const [mapPanelOpen, setMapPanelOpen] = useState(showOwnershipMapByDefault);
	const client = useMemo(() => createTauriClient(), []);
	const kubeconfigEnvVar = useSettingsState((state) => state.kubeconfigSourceKey);
	const renderCountRef = useRef(0);
	renderCountRef.current += 1;

	const namespaceKey = selectedNamespaces.join(",");
	const kindKey = selectedKinds.map(resourceKindFetchKey).join(",");
	const scopeStateKey = `${clusterContext}|${namespaceKey}|${kindKey}|${selectedArgoAppFilter}|${healthFilter}`;
	const [prevScopeStateKey, setPrevScopeStateKey] = useState(scopeStateKey);
	const resetStateKey = `${clusterContext}|${namespaceKey}|${kindKey}|${initialHealthFilter}`;
	const [prevResetStateKey, setPrevResetStateKey] = useState(resetStateKey);
	const [prevInitialHealthFilter, setPrevInitialHealthFilter] =
		useState(initialHealthFilter);
	const [prevInitialSearch, setPrevInitialSearch] = useState(initialSearch);

	if (prevResetStateKey !== resetStateKey) {
		setPrevResetStateKey(resetStateKey);
		setCollapsedGroups(new Set());
		setHealthFilter(initialHealthFilter);
	}
	if (prevScopeStateKey !== scopeStateKey) {
		setPrevScopeStateKey(scopeStateKey);
		setPageIndex(0);
	}
	if (prevInitialHealthFilter !== initialHealthFilter) {
		setPrevInitialHealthFilter(initialHealthFilter);
		setHealthFilter(initialHealthFilter);
		setPageIndex(0);
	}
	if (prevInitialSearch !== initialSearch) {
		setPrevInitialSearch(initialSearch);
		setSearch(initialSearch);
		setPageIndex(0);
	}
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
	const topologyFitViewKey = useMemo(
		() => JSON.stringify(topologyQueryKey),
		[topologyQueryKey],
	);

	const { data, isPending, isError, error } = useQuery({
		queryKey,
		queryFn: () => fetchResourcePage(clusterContext, fetchKeys, kubeconfigEnvVar),
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
			listResourceTopology(
				client,
				clusterContext,
				topologyNamespaces,
				topologyMode,
				kubeconfigEnvVar,
			),
		enabled: Boolean(clusterContext && mapPanelOpen),
		staleTime: 30_000,
		placeholderData: keepPreviousData,
	});
	const { data: metricsData, isError: metricsError } = useQuery({
		queryKey: queryKeys.resourceMetrics(
			clusterContext,
			topologyNamespaces,
			kubeconfigEnvVar,
		),
		queryFn: () =>
			listResourceMetrics(client, clusterContext, topologyNamespaces, kubeconfigEnvVar),
		enabled: Boolean(clusterContext),
		retry: false,
		staleTime: 30_000,
		placeholderData: keepPreviousData,
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
	const argoApps = useMemo(() => uniqueArgoApps(dataWithMetrics), [dataWithMetrics]);
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
	const groupedByArgo = useMemo(
		() => filteredData.some((resource) => resource.argoApp || resource.ownerRef),
		[filteredData],
	);
	const displayData = useMemo(() => {
		if (!groupedByArgo) return sortedData;
		return sortedData.toSorted((a, b) => {
			const appCompare = formatResourceGroupLabel(a).localeCompare(
				formatResourceGroupLabel(b),
			);
			if (appCompare !== 0) return appCompare;
			const kindRankCompare =
				resourceGroupKindRank(a.kind) - resourceGroupKindRank(b.kind);
			if (kindRankCompare !== 0) return kindRankCompare;
			const kindCompare = a.kind.localeCompare(b.kind);
			if (kindCompare !== 0) return kindCompare;
			return a.name.localeCompare(b.name);
		});
	}, [groupedByArgo, sortedData]);
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
		() => pageAppGroupCounts(pageRows, groupedByArgo),
		[groupedByArgo, pageRows],
	);
	const pageTypeGroups = useMemo(
		() => pageTypeGroupCounts(pageRows, groupedByArgo),
		[groupedByArgo, pageRows],
	);

	// Hide columns that would render "—" for every visible row so the name
	// column keeps its width for what actually matters.
	const columnVisibility = useMemo(
		() => ({
			ready: pageRows.some((row) => Boolean(row.ready)),
			restarts: pageRows.some((row) => row.restarts !== undefined),
			cpu: pageRows.some((row) => row.metrics?.cpuMillicores !== undefined),
			memory: pageRows.some((row) => row.metrics?.memoryBytes !== undefined),
			ownerRef: pageRows.some((row) => Boolean(row.ownerRef)),
			"argo-helm": pageRows.some(
				(row) => Boolean(row.argoApp) || Boolean(row.helmRelease),
			),
		}),
		[pageRows],
	);

	const table = useReactTable({
		data: pageRows,
		columns,
		state: { sorting, columnVisibility },
		onSortingChange: setSorting,
		getCoreRowModel: getCoreRowModel(),
	});
	const externalSelectedResourceKey = selectedResource
		? resourceSelectionKey(selectedResource)
		: null;
	const externalSelectedResourceIdentityKey = selectedResource
		? resourceIdentityKey(selectedResource)
		: null;
	const activeSelectedResourceKey =
		selectedResourceKey ?? externalSelectedResourceKey;
	const activeSelectedResourceIdentityKey =
		selectedResourceIdentityKey ?? externalSelectedResourceIdentityKey;

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
		const appCollapseKey = resourceGroupCollapseKey(selectedRow);
		const typeCollapseKey = resourceTypeGroupCollapseKey(selectedRow);
		if (
			!collapsedGroups.has(appCollapseKey) &&
			!collapsedGroups.has(typeCollapseKey)
		) {
			return collapsedGroups;
		}
		const next = new Set(collapsedGroups);
		next.delete(appCollapseKey);
		next.delete(typeCollapseKey);
		return next;
	}, [
		activeSelectedResourceIdentityKey,
		activeSelectedResourceKey,
		collapsedGroups,
		displayData,
	]);

	useEffect(() => {
		if (!activeSelectedResourceKey && !activeSelectedResourceIdentityKey) return;
		const selectedRow = displayData.find(
			(resource) =>
				resourceSelectionKey(resource) === activeSelectedResourceKey ||
				resourceIdentityKey(resource) === activeSelectedResourceIdentityKey,
		);
		if (!selectedRow) return;
		const appCollapseKey = resourceGroupCollapseKey(selectedRow);
		const typeCollapseKey = resourceTypeGroupCollapseKey(selectedRow);
		setCollapsedGroups((current) => {
			if (!current.has(appCollapseKey) && !current.has(typeCollapseKey)) {
				return current;
			}
			const next = new Set(current);
			next.delete(appCollapseKey);
			next.delete(typeCollapseKey);
			return next;
		});
	}, [activeSelectedResourceIdentityKey, activeSelectedResourceKey, displayData]);

	useEffect(() => {
		if (externalSelectedResourceKey) {
			setSelectedResourceKey(externalSelectedResourceKey);
		}
		if (externalSelectedResourceIdentityKey) {
			setSelectedResourceIdentityKey(externalSelectedResourceIdentityKey);
		}
	}, [externalSelectedResourceKey, externalSelectedResourceIdentityKey]);

	useEffect(() => {
		diagnosticLog("resources.render", {
			render: renderCountRef.current,
			pending: isPending,
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
		setSelectedResourceKey(resourceSelectionKey(resource));
		setSelectedResourceIdentityKey(resourceIdentityKey(resource));
		onResourceSelect(resource);
	};
	const handleTopologyNodeSelect = (
		node: TopologyNode,
		resource: ResourceSummary | null,
	) => {
		setSelectedTopologyNodeId(node.id);
		if (!resource) return;
		setSelectedResourceKey(resourceSelectionKey(resource));
		setSelectedResourceIdentityKey(resourceIdentityKey(resource));
		onResourceSelect(resource);
	};
	const handleMapPanelOpenChange = (open: boolean) => {
		setMapPanelOpen(open);
		if (!open) setSelectedTopologyNodeId(null);
	};

	if (isPending) {
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

	return (
		<div className="flex h-full min-h-0 min-w-0 flex-col gap-3">
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
				onFilterChange={setHealthFilter}
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
				onReset={() => setHealthFilter("all")}
			/>
			<ResourceToolbar
				search={search}
				argoApps={argoApps}
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
			<ResourceMapTableLayout
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
				onMapPanelOpenChange={handleMapPanelOpenChange}
				onTopologyNodeSelect={handleTopologyNodeSelect}
				table={table}
				groupedByArgo={groupedByArgo}
				pageGroups={pageGroups}
				pageTypeGroups={pageTypeGroups}
				collapsedGroups={effectiveCollapsedGroups}
				selectedResourceKey={activeSelectedResourceKey}
				selectedResourceIdentityKey={activeSelectedResourceIdentityKey}
				onToggleGroup={toggleGroup}
				onSelectedResourceKeyChange={setSelectedResourceKey}
				onResourceSelect={handleResourceSelect}
				totalRows={totalRows}
				search={search}
				pageIndex={safePageIndex}
				pageCount={pageCount}
				onPageChange={setPageIndex}
				realtime={realtime}
			/>
		</div>
	);
}

export const ResourceList = memo(ResourceListComponent);
