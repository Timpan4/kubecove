import { memo, useEffect, useMemo, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import {
	getCoreRowModel,
	useReactTable,
	type SortingState,
} from "@tanstack/react-table";
import { diagnosticLog } from "@/lib/diagnostics";
import type { ResourceKindSelection, ResourceSummary } from "@/lib/types";
import { columns } from "./columns";
import { ERROR_STATE_CLASS, PAGE_SIZE, STATE_CLASS } from "./constants";
import { pageAppGroupCounts, pageTypeGroupCounts } from "./grouping";
import {
	buildFetchKeys,
	buildResourceHealthSummary,
	describeResourceScope,
	filterResources,
	formatResourceGroupLabel,
	resourceKindFetchKey,
	sortedRows,
	uniqueArgoApps,
} from "./helpers";
import { ResourceHealthStrip, ResourceScopePills } from "./health";
import { ResourcePagination } from "./pagination";
import { fetchResourcePage } from "./query";
import { ResourceTable } from "./ResourceTable";
import { ResourceToolbar } from "./toolbar";

interface ResourceListProps {
	clusterContext: string;
	selectedNamespaces: string[];
	selectedKinds: ResourceKindSelection[];
	selectedArgoAppFilter: string;
	onArgoAppFilterChange: (app: string) => void;
	onResourceSelect: (resource: ResourceSummary) => void;
}

function ResourceListComponent({
	clusterContext,
	selectedNamespaces,
	selectedKinds,
	selectedArgoAppFilter,
	onArgoAppFilterChange,
	onResourceSelect,
}: ResourceListProps) {
	const [pageIndex, setPageIndex] = useState(0);
	const [sorting, setSorting] = useState<SortingState>([]);
	const [search, setSearch] = useState("");
	const [selectedResourceKey, setSelectedResourceKey] = useState<string | null>(
		null,
	);
	const [collapsedGroups, setCollapsedGroups] = useState<Set<string>>(
		() => new Set(),
	);
	const renderCountRef = useRef(0);
	renderCountRef.current += 1;

	const namespaceKey = selectedNamespaces.join(",");
	const kindKey = selectedKinds.map(resourceKindFetchKey).join(",");
	const fetchKeys = useMemo(
		() => buildFetchKeys(selectedNamespaces, selectedKinds),
		[namespaceKey, kindKey],
	);
	const queryKey = useMemo(
		() =>
			[
				"resources",
				clusterContext,
				...fetchKeys.map((key) => `${resourceKindFetchKey(key.kind)}:${key.namespace ?? ""}`),
			] as const,
		[clusterContext, fetchKeys],
	);

	useEffect(() => {
		setPageIndex(0);
	}, [clusterContext, namespaceKey, kindKey, selectedArgoAppFilter]);

	useEffect(() => {
		setCollapsedGroups(new Set());
	}, [clusterContext, namespaceKey, kindKey]);

	const { data, isPending, isError, error } = useQuery({
		queryKey,
		queryFn: () => fetchResourcePage(clusterContext, fetchKeys),
		enabled: fetchKeys.length > 0,
		staleTime: 30_000,
	});

	const argoApps = useMemo(() => uniqueArgoApps(data ?? []), [data]);
	const filteredData = useMemo(
		() => filterResources(data ?? [], search, selectedArgoAppFilter),
		[data, search, selectedArgoAppFilter],
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
		() => buildResourceHealthSummary(filteredData),
		[filteredData],
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

	useEffect(() => {
		diagnosticLog("resources.render", {
			render: renderCountRef.current,
			pending: isPending,
			error: isError,
			rawRows: data?.length ?? 0,
			pageRows: pageRows.length,
			fetches: fetchKeys.length,
			selected: selectedResourceKey ?? "",
		});
	});

	const clearFilters = () => {
		setSearch("");
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

	if (isPending) {
		return (
			<div className={STATE_CLASS}>
				<div className="mb-4 flex flex-col gap-px">
					{Array.from({ length: 8 }).map((_, i) => (
						<div key={i} className="flex gap-3 border-b py-2">
							<div className="h-3.5 w-[180px] animate-pulse rounded-sm bg-muted"></div>
							<div className="h-3.5 w-[100px] animate-pulse rounded-sm bg-muted"></div>
							<div className="h-3.5 w-20 animate-pulse rounded-sm bg-muted"></div>
							<div className="h-3.5 w-[70px] animate-pulse rounded-sm bg-muted"></div>
						</div>
					))}
				</div>
				<span className="inline-flex items-center gap-2">Loading resources…</span>
			</div>
		);
	}

	if (isError) {
		return (
			<div className={ERROR_STATE_CLASS}>
				<span>
					Error:{" "}
					{error instanceof Error ? error.message : "Failed to load resources"}
				</span>
			</div>
		);
	}

	if (!data || data.length === 0) {
		return (
			<div className={STATE_CLASS}>
				<span>No resources found</span>
			</div>
		);
	}

	return (
		<div className="flex min-w-0 flex-col gap-3">
			<ResourceScopePills pills={scopePills} />
			<ResourceHealthStrip summary={healthSummary} />
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
			<ResourceTable
				table={table}
				groupedByArgo={groupedByArgo}
				pageGroups={pageGroups}
				pageTypeGroups={pageTypeGroups}
				collapsedGroups={collapsedGroups}
				selectedResourceKey={selectedResourceKey}
				onToggleGroup={toggleGroup}
				onSelectedResourceKeyChange={setSelectedResourceKey}
				onResourceSelect={onResourceSelect}
			/>
			<ResourcePagination
				totalRows={totalRows}
				search={search}
				pageIndex={safePageIndex}
				pageCount={pageCount}
				onPageChange={setPageIndex}
			/>
		</div>
	);
}

export const ResourceList = memo(ResourceListComponent);
