import { Fragment, memo, useEffect, useMemo, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import {
	flexRender,
	getCoreRowModel,
	useReactTable,
	type SortingState,
} from "@tanstack/react-table";
import { createTauriClient, listResources } from "@/lib/tauri";
import type { ResourceSummary } from "@/lib/types";
import { diagnosticLog } from "@/lib/diagnostics";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
	Select,
	SelectContent,
	SelectItem,
	SelectTrigger,
	SelectValue,
} from "@/components/ui/select";
import { ChevronDown, ChevronRight, Search, X } from "lucide-react";
import { cn } from "@/lib/utils";
import { columns } from "./columns";
import {
	getResourceGroupVisual,
	getResourceKindVisual,
} from "@/lib/resource-visuals";
import {
	buildFetchKeys,
	buildResourceHealthSummary,
	describeResourceScope,
	filterResources,
	formatResourceGroupLabel,
	formatResourceTypeGroupLabel,
	resourceGroupCollapseKey,
	resourceTypeGroupCollapseKey,
	sortedRows,
	uniqueArgoApps,
	type FetchKey,
} from "./helpers";

interface ResourceListProps {
	clusterContext: string;
	selectedNamespaces: string[];
	selectedKinds: string[];
	selectedArgoAppFilter: string;
	onArgoAppFilterChange: (app: string) => void;
	onResourceSelect: (resource: ResourceSummary) => void;
}

async function fetchResourcePage(
	clusterContext: string,
	fetchKeys: FetchKey[],
): Promise<ResourceSummary[]> {
	const started = performance.now();
	diagnosticLog("resources.fetch.start", {
		cluster: clusterContext,
		fetches: fetchKeys.length,
		kinds: fetchKeys.map((key) => key.kind).join("|"),
	});
	const client = createTauriClient();
	const results = await Promise.all(
		fetchKeys.map(({ kind, namespace }) =>
			listResources(
				client,
				clusterContext,
				kind,
				namespace === "" ? undefined : namespace,
			),
		),
	);
	const rows = results.flat();
	diagnosticLog("resources.fetch.done", {
		cluster: clusterContext,
		fetches: fetchKeys.length,
		rows: rows.length,
		ms: Math.round(performance.now() - started),
	});
	return rows;
}

const PAGE_SIZE = 50;
const TABLE_CLASS =
	"min-w-[1120px] table-fixed border-collapse text-sm [&_th:nth-child(1)]:w-[27%] [&_td:nth-child(1)]:w-[27%] [&_th:nth-child(2)]:w-[11%] [&_td:nth-child(2)]:w-[11%] [&_th:nth-child(3)]:w-[12%] [&_td:nth-child(3)]:w-[12%] [&_th:nth-child(4)]:w-[10%] [&_td:nth-child(4)]:w-[10%] [&_th:nth-child(5)]:w-[7%] [&_td:nth-child(5)]:w-[7%] [&_th:nth-child(6)]:w-[9%] [&_td:nth-child(6)]:w-[9%] [&_th:nth-child(7)]:w-[14%] [&_td:nth-child(7)]:w-[14%] [&_th:nth-child(8)]:w-[5%] [&_td:nth-child(8)]:w-[5%] [&_th:nth-child(9)]:w-[7%] [&_td:nth-child(9)]:w-[7%] [&_th]:border-b-2 [&_th]:px-3 [&_th]:py-3 [&_th]:text-left [&_th]:text-xs [&_th]:font-semibold [&_th]:uppercase [&_th]:text-muted-foreground [&_td]:whitespace-nowrap [&_td]:border-b [&_td]:px-3 [&_td]:py-3";
const ROW_CLASS =
	"cursor-pointer transition-colors hover:bg-accent/60";
const SELECTED_ROW_CLASS = "bg-accent";
const STATE_CLASS = "p-8 text-center text-sm text-muted-foreground";
const ERROR_STATE_CLASS = "p-8 text-center text-sm text-destructive";
const EMPTY_PAGE_CLASS = "p-8 text-center text-sm text-muted-foreground";
const TOOLBAR_CLASS = "mb-1 flex items-center gap-2 p-0";
const PAGINATION_CLASS =
	"flex items-center justify-between border-t py-2 text-xs text-muted-foreground";

function HealthMetric({
	label,
	value,
	valueClassName,
}: {
	label: string;
	value: number;
	valueClassName?: string;
}) {
	return (
		<div className="flex min-h-14 flex-col justify-center gap-1 rounded-md border bg-card p-3">
			<span className="text-[0.72rem] font-semibold uppercase text-muted-foreground">
				{label}
			</span>
			<strong className={valueClassName}>{value}</strong>
		</div>
	);
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

	// Stable query key: serialize fetchKeys as sorted JSON string
	const fetchKeys = useMemo(
		() => buildFetchKeys(selectedNamespaces, selectedKinds),
		[selectedNamespaces.join(","), selectedKinds.join(",")],
	);
	const queryKey = useMemo(
		() =>
			[
				"resources",
				clusterContext,
				...fetchKeys.map((k) => `${k.kind}:${k.namespace}`),
			] as const,
		[clusterContext, fetchKeys],
	);

	// Reset page index when filters change
	useEffect(() => {
		setPageIndex(0);
	}, [
		clusterContext,
		selectedNamespaces.join(","),
		selectedKinds.join(","),
		selectedArgoAppFilter,
	]);

	useEffect(() => {
		setCollapsedGroups(new Set());
	}, [clusterContext, selectedNamespaces.join(","), selectedKinds.join(",")]);

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
		() => filteredData.some((resource) => resource.argoApp),
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

	// Clamp pageIndex to valid range
	const safePageIndex = Math.min(pageIndex, Math.max(0, pageCount - 1));
	const startRow = safePageIndex * PAGE_SIZE;
	const endRow = startRow + PAGE_SIZE;
	const pageRows = useMemo(
		() => displayData.slice(startRow, endRow),
		[displayData, startRow, endRow],
	);
	const pageGroups = useMemo(() => {
		const counts = new Map<string, number>();
		if (!groupedByArgo) return counts;
		for (const row of pageRows) {
			const label = formatResourceGroupLabel(row);
			counts.set(label, (counts.get(label) ?? 0) + 1);
		}
		return counts;
	}, [groupedByArgo, pageRows]);
	const pageTypeGroups = useMemo(() => {
		const counts = new Map<string, number>();
		if (!groupedByArgo) return counts;
		for (const row of pageRows) {
			const key = `${formatResourceGroupLabel(row)}::${formatResourceTypeGroupLabel(row)}`;
			counts.set(key, (counts.get(key) ?? 0) + 1);
		}
		return counts;
	}, [groupedByArgo, pageRows]);

	const table = useReactTable({
		data: pageRows,
		columns,
		state: { sorting },
		onSortingChange: setSorting,
		getCoreRowModel: getCoreRowModel(),
	});
	const rowModel = table.getRowModel();

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
			<div className="flex min-h-8 flex-wrap items-center gap-2" aria-label="Current resource scope">
				{scopePills.map((pill) => (
					<Badge
						key={pill.label}
						variant="outline"
						className="h-8 max-w-full gap-1.5 rounded-sm border-slate-700/80 bg-slate-950/45 px-2.5 text-xs shadow-none"
					>
						<span className="text-muted-foreground">{pill.label}</span>
						<strong className="min-w-0 truncate font-semibold text-foreground">
							{pill.value}
						</strong>
					</Badge>
				))}
			</div>
			<div className="grid grid-cols-1 gap-2 md:grid-cols-5" aria-label="Resource health summary">
				<HealthMetric label="Total" value={healthSummary.total} />
				<HealthMetric
					label="Healthy"
					value={healthSummary.healthy}
					valueClassName="text-emerald-300"
				/>
				<HealthMetric
					label="Needs attention"
					value={healthSummary.attention}
					valueClassName="text-amber-300"
				/>
				<HealthMetric
					label="Degraded"
					value={healthSummary.degraded}
					valueClassName="text-red-300"
				/>
				<HealthMetric
					label="Restarted"
					value={healthSummary.restarted}
					valueClassName="text-sky-300"
				/>
			</div>
			<div className={TOOLBAR_CLASS}>
				<div className="relative min-w-0 flex-1">
					<Search className="pointer-events-none absolute left-2.5 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
					<Input
						className="h-9 border-slate-700/80 bg-slate-950/45 pl-8 text-sm text-foreground placeholder:text-muted-foreground"
						type="text"
						placeholder="Search by name, namespace, kind, owner, Argo app, Helm release..."
						value={search}
						onChange={(e) => {
							setSearch(e.target.value);
							setPageIndex(0);
						}}
					/>
				</div>
				{argoApps.length > 0 && (
					<Select
						value={selectedArgoAppFilter || "all"}
						onValueChange={(value) => {
							onArgoAppFilterChange(value === "all" ? "" : value);
							setPageIndex(0);
						}}
					>
						<SelectTrigger
							className="h-9 max-w-52 border-slate-700/80 bg-slate-950/45 text-foreground"
							aria-label="Filter by Argo application"
						>
							<SelectValue placeholder="All Argo apps" />
						</SelectTrigger>
						<SelectContent>
							<SelectItem value="all">All Argo apps</SelectItem>
							{argoApps.map((app) => (
								<SelectItem key={app} value={app}>
									{app}
								</SelectItem>
							))}
						</SelectContent>
					</Select>
				)}
				{(search || selectedArgoAppFilter) && (
					<Button
						type="button"
						variant="outline"
						size="sm"
						onClick={() => {
							setSearch("");
							onArgoAppFilterChange("");
							setPageIndex(0);
						}}
					>
						<X className="size-3.5" />
						Clear
					</Button>
				)}
				{(search || selectedArgoAppFilter) && filteredData.length === 0 && (
					<span className="text-xs text-muted-foreground">
						No results for current filters{" "}
						<Button
							type="button"
							variant="link"
							size="sm"
							className="h-auto px-0 py-0 text-xs"
							onClick={() => {
								setSearch("");
								onArgoAppFilterChange("");
								setPageIndex(0);
							}}
						>
							clear filters
						</Button>
					</span>
				)}
			</div>

			<div className="w-full max-w-full min-w-0 overflow-x-auto">
				<table className={TABLE_CLASS}>
				<thead>
					{table.getHeaderGroups().map((headerGroup) => (
						<tr key={headerGroup.id}>
							{headerGroup.headers.map((header) => (
								<th
									key={header.id}
									onClick={header.column.getToggleSortingHandler()}
									className={header.column.getCanSort() ? "cursor-pointer" : "cursor-default"}
								>
									{flexRender(
										header.column.columnDef.header,
										header.getContext(),
									)}
									{header.column.getIsSorted() === "asc"
										? " ↑"
										: header.column.getIsSorted() === "desc"
											? " ↓"
											: ""}
								</th>
							))}
						</tr>
					))}
				</thead>
				<tbody>
					{pageRows.length === 0 ? (
						<tr>
							<td colSpan={columns.length} className={EMPTY_PAGE_CLASS}>
								No resources match your filter
							</td>
						</tr>
					) : (
						rowModel.rows.map((row, index) => {
							const resourceKey = `${row.original.cluster}:${row.original.kind}:${row.original.namespace ?? ""}:${row.original.name}`;
							const isSelected = selectedResourceKey === resourceKey;
							const previous =
								index > 0
									? rowModel.rows[index - 1]?.original
									: null;
							const showGroupHeader =
								groupedByArgo &&
								(!previous ||
									formatResourceGroupLabel(previous) !==
										formatResourceGroupLabel(row.original));
							const label = formatResourceGroupLabel(row.original);
							const typeLabel = formatResourceTypeGroupLabel(row.original);
							const typeKey = `${label}::${typeLabel}`;
							const appCollapseKey = resourceGroupCollapseKey(row.original);
							const typeCollapseKey = resourceTypeGroupCollapseKey(row.original);
							const appCollapsed = collapsedGroups.has(appCollapseKey);
							const typeCollapsed = collapsedGroups.has(typeCollapseKey);
							const appVisual = getResourceGroupVisual(label);
							const AppIcon = appVisual.icon;
							const typeVisual = getResourceKindVisual(row.original.kind);
							const TypeIcon = typeVisual.icon;
							const showTypeGroupHeader =
								groupedByArgo &&
								(!previous ||
									formatResourceGroupLabel(previous) !== label ||
									formatResourceTypeGroupLabel(previous) !== typeLabel);
							const hideResourceRow =
								groupedByArgo && (appCollapsed || typeCollapsed);
							return (
								<Fragment key={row.id}>
									{showGroupHeader && (
										<tr className="[&_td]:bg-muted/50 [&_td]:p-0 [&_td]:text-xs [&_td]:font-bold [&_td]:text-primary">
											<td colSpan={columns.length} className="!p-0">
												<button
													type="button"
													className="flex w-full cursor-pointer items-center gap-2 border-0 bg-muted/50 px-3 py-2 text-left text-inherit focus-visible:ring-1 focus-visible:ring-ring/50"
													onClick={() => toggleGroup(appCollapseKey)}
													aria-expanded={!appCollapsed}
												>
													{appCollapsed ? (
														<ChevronRight className="size-4 shrink-0 text-muted-foreground" />
													) : (
														<ChevronDown className="size-4 shrink-0 text-muted-foreground" />
													)}
													<AppIcon
														className={cn("size-3.5 shrink-0", appVisual.className)}
													/>
													<span className="ml-0 text-muted-foreground">{label}</span>
													<small className="ml-0.5 text-[0.6875rem] font-medium text-muted-foreground">
														{pageGroups.get(label) ?? 0} resources on this page
													</small>
												</button>
											</td>
										</tr>
									)}
									{showTypeGroupHeader && !appCollapsed && (
										<tr className="[&_td]:bg-card [&_td]:p-0 [&_td]:text-[0.72rem] [&_td]:font-bold [&_td]:uppercase [&_td]:text-foreground">
											<td colSpan={columns.length} className="!p-0">
												<button
													type="button"
													className="flex w-full cursor-pointer items-center gap-2 border-0 bg-card py-1.5 pl-6 pr-3 text-left text-[0.6875rem] text-inherit focus-visible:ring-1 focus-visible:ring-ring/50"
													onClick={() => toggleGroup(typeCollapseKey)}
													aria-expanded={!typeCollapsed}
												>
													{typeCollapsed ? (
														<ChevronRight className="size-4 shrink-0 text-muted-foreground" />
													) : (
														<ChevronDown className="size-4 shrink-0 text-muted-foreground" />
													)}
													<TypeIcon
														className={cn("size-3.5 shrink-0", typeVisual.className)}
													/>
													<span className="ml-0 text-inherit">{typeLabel}</span>
													<small className="ml-0.5 text-[0.625rem] font-medium normal-case text-muted-foreground">
														{pageTypeGroups.get(typeKey) ?? 0} on this page
													</small>
												</button>
											</td>
										</tr>
									)}
									{!hideResourceRow && (
										<tr
											className={cn(ROW_CLASS, isSelected && SELECTED_ROW_CLASS)}
											onClick={() => {
												diagnosticLog("resources.row.click", {
													key: resourceKey,
													alreadySelected: isSelected,
													rowIndex: index,
												});
												setSelectedResourceKey(resourceKey);
												onResourceSelect(row.original);
											}}
										>
											{row.getVisibleCells().map((cell) => (
												<td key={cell.id}>
													{flexRender(
														cell.column.columnDef.cell,
														cell.getContext(),
													)}
												</td>
											))}
										</tr>
									)}
								</Fragment>
							);
						})
					)}
				</tbody>
				</table>
			</div>

			<div className={PAGINATION_CLASS}>
				<Button
					type="button"
					variant="outline"
					size="sm"
					onClick={() => setPageIndex((p) => Math.max(0, p - 1))}
					disabled={safePageIndex === 0}
				>
					Previous
				</Button>
				<span>
					{totalRows} {search ? "filtered" : "total"} rows
				</span>
				<span>
					Page {safePageIndex + 1} of {pageCount}
				</span>
				<Button
					type="button"
					variant="outline"
					size="sm"
					onClick={() => setPageIndex((p) => Math.min(pageCount - 1, p + 1))}
					disabled={safePageIndex >= pageCount - 1}
				>
					Next
				</Button>
			</div>
		</div>
	);
}

export const ResourceList = memo(ResourceListComponent);
