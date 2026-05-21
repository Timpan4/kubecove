import { memo, useEffect, useMemo, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import {
	getCoreRowModel,
	useReactTable,
	type SortingState,
} from "@tanstack/react-table";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import {
	Empty,
	EmptyDescription,
	EmptyHeader,
	EmptyTitle,
} from "@/components/ui/empty";
import { Skeleton } from "@/components/ui/skeleton";
import { Spinner } from "@/components/ui/spinner";
import { diagnosticLog } from "@/lib/diagnostics";
import { queryKeys } from "@/lib/queryKeys";
import { createTauriClient, listResourceTopology } from "@/lib/tauri";
import type {
	ResourceKindSelection,
	ResourceSummary,
	TopologyNode,
} from "@/lib/types";
import { columns } from "./columns";
import { PAGE_SIZE } from "./constants";
import { pageAppGroupCounts, pageTypeGroupCounts } from "./grouping";
import {
	buildFetchKeys,
	buildResourceHealthSummary,
	describeResourceScope,
	filterResourcesByHealth,
	filterResources,
	formatResourceGroupLabel,
	resourceIdentityKey,
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
	ResourceScopePills,
} from "./health";
import { fetchResourcePage } from "./query";
import { ResourceMapTableLayout } from "./ResourceMapTableLayout";
import { ResourceToolbar } from "./toolbar";
import { useResourceWatch } from "./useResourceWatch";

interface ResourceListProps {
	clusterContext: string;
	selectedNamespaces: string[];
	selectedKinds: ResourceKindSelection[];
	selectedArgoAppFilter: string;
	selectedResource: ResourceSummary | null;
	initialHealthFilter?: HealthFilter;
	onArgoAppFilterChange: (app: string) => void;
	onResourceSelect: (resource: ResourceSummary) => void;
}

function ResourceListComponent({
	clusterContext,
	selectedNamespaces,
	selectedKinds,
	selectedArgoAppFilter,
	selectedResource,
	initialHealthFilter = "all",
	onArgoAppFilterChange,
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
	const [mapPanelOpen, setMapPanelOpen] = useState(true);
	const client = useMemo(() => createTauriClient(), []);
	const renderCountRef = useRef(0);
	renderCountRef.current += 1;

	const namespaceKey = selectedNamespaces.join(",");
	const kindKey = selectedKinds.map(resourceKindFetchKey).join(",");
	const topologyNamespaceKey = useMemo(
		() =>
			[...new Set(selectedNamespaces)]
				.sort((a, b) => a.localeCompare(b))
				.join(","),
		[namespaceKey],
	);
	const topologyNamespaces = useMemo(
		() => (topologyNamespaceKey ? topologyNamespaceKey.split(",") : []),
		[topologyNamespaceKey],
	);
	const fetchKeys = useMemo(
		() => buildFetchKeys(selectedNamespaces, selectedKinds),
		[namespaceKey, kindKey],
	);
	const queryKey = useMemo(
		() => queryKeys.resources(clusterContext, fetchKeys),
		[clusterContext, fetchKeys],
	);
	const topologyQueryKey = useMemo(
		() => queryKeys.resourceTopology(clusterContext, topologyNamespaces),
		[clusterContext, topologyNamespaces],
	);
	const topologyFitViewKey = useMemo(
		() => JSON.stringify(topologyQueryKey),
		[topologyQueryKey],
	);

	useEffect(() => {
		setPageIndex(0);
	}, [clusterContext, namespaceKey, kindKey, selectedArgoAppFilter, healthFilter]);

	useEffect(() => {
		setHealthFilter(initialHealthFilter);
		setPageIndex(0);
	}, [initialHealthFilter]);

	useEffect(() => {
		setCollapsedGroups(new Set());
		setHealthFilter(initialHealthFilter);
	}, [clusterContext, namespaceKey, kindKey, initialHealthFilter]);

	const { data, isPending, isError, error } = useQuery({
		queryKey,
		queryFn: () => fetchResourcePage(clusterContext, fetchKeys),
		enabled: fetchKeys.length > 0,
		staleTime: 30_000,
	});
	const topologyQuery = useQuery({
		queryKey: topologyQueryKey,
		queryFn: () => listResourceTopology(client, clusterContext, topologyNamespaces),
		enabled: Boolean(clusterContext && mapPanelOpen),
		staleTime: 30_000,
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
		subscriptions: watchSubscriptions,
		enabled:
			watchSubscriptions.some((subscription) => subscription.keys.length > 0) &&
			!isError,
	});

	const argoApps = useMemo(() => uniqueArgoApps(data ?? []), [data]);
	const scopedData = useMemo(
		() => filterResources(data ?? [], search, selectedArgoAppFilter),
		[data, search, selectedArgoAppFilter],
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
		return [...sortedData].sort((a, b) => {
			const appCompare = formatResourceGroupLabel(a).localeCompare(
				formatResourceGroupLabel(b),
			);
			if (appCompare !== 0) return appCompare;
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
				clusterContext,
				selectedNamespaces,
				selectedKinds,
				selectedArgoAppFilter,
			),
		[clusterContext, selectedNamespaces, selectedKinds, selectedArgoAppFilter],
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

	const table = useReactTable({
		data: pageRows,
		columns,
		state: { sorting },
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
		const topologyNodes = topologyQuery.data?.nodes;
		if (!topologyNodes) return selectedTopologyNodeId;
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
		topologyQuery.data,
	]);
	useEffect(() => {
		if (!selectedTopologyNodeId || !topologyQuery.data) return;
		if (topologyQuery.data.nodes.some((node) => node.id === selectedTopologyNodeId)) {
			return;
		}
		setSelectedTopologyNodeId(null);
	}, [selectedTopologyNodeId, topologyQuery.data]);
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
					Loading resources...
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
		return (
			<Empty className="min-h-64 border-0">
				<EmptyHeader>
					<EmptyTitle>No resources found</EmptyTitle>
					<EmptyDescription>
						Try a different namespace or resource kind selection.
					</EmptyDescription>
				</EmptyHeader>
			</Empty>
		);
	}

	return (
		<div className="flex min-w-0 flex-col gap-3">
			<ResourceScopePills pills={scopePills} />
			<ResourceHealthStrip
				summary={healthSummary}
				activeFilter={healthFilter}
				onFilterChange={setHealthFilter}
			/>
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
				topology={topologyQuery.data}
				topologyLoading={topologyQuery.isPending}
				topologyError={topologyQuery.isError}
				topologyErr={topologyQuery.error}
				selectedTopologyNodeId={syncedTopologyNodeId}
				topologyFitViewKey={topologyFitViewKey}
				mapPanelOpen={mapPanelOpen}
				onMapPanelOpenChange={handleMapPanelOpenChange}
				onTopologyNodeSelect={handleTopologyNodeSelect}
				table={table}
				groupedByArgo={groupedByArgo}
				pageGroups={pageGroups}
				pageTypeGroups={pageTypeGroups}
				collapsedGroups={collapsedGroups}
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
