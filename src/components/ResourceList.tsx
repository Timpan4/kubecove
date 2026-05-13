import { Fragment, memo, useEffect, useMemo, useRef, useState } from "react";
import type { ReactNode } from "react";
import { useQuery } from "@tanstack/react-query";
import {
	createColumnHelper,
	flexRender,
	getCoreRowModel,
	useReactTable,
	type SortingState,
} from "@tanstack/react-table";
import { createTauriClient, listResources } from "../lib/tauri";
import type { ResourceSummary, ClusterScopedKind } from "../lib/types";
import { CLUSTER_SCOPED_KINDS } from "../lib/types";
import { diagnosticLog } from "../lib/diagnostics";
import {
	Tooltip,
	TooltipContent,
	TooltipTrigger,
} from "@/components/ui/tooltip";

interface ResourceListProps {
	clusterContext: string;
	selectedNamespaces: string[];
	selectedKinds: string[];
	selectedArgoAppFilter: string;
	onArgoAppFilterChange: (app: string) => void;
	onResourceSelect: (resource: ResourceSummary) => void;
}

interface FetchKey {
	kind: string;
	namespace: string | undefined; // undefined means all namespaces or cluster-scoped
}

interface HealthSummary {
	total: number;
	healthy: number;
	attention: number;
	degraded: number;
	restarted: number;
}

interface ScopePill {
	label: string;
	value: string;
}

function isClusterScopedKind(kind: string): kind is ClusterScopedKind {
	return (CLUSTER_SCOPED_KINDS as readonly string[]).includes(kind);
}

function buildFetchKeys(namespaces: string[], kinds: string[]): FetchKey[] {
	const keys: FetchKey[] = [];
	for (const k of kinds) {
		if (isClusterScopedKind(k)) {
			// Cluster-scoped kinds have no namespace
			keys.push({ kind: k, namespace: undefined });
		} else {
			// Namespaced kinds require explicit selected namespaces.
			for (const ns of namespaces) {
				keys.push({ kind: k, namespace: ns });
			}
		}
	}
	return keys;
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

const columnHelper = createColumnHelper<ResourceSummary>();

type ChipVariant = "neutral" | "success" | "warning" | "error" | "info";

function StatusChip({
	value,
	variant = "neutral",
}: {
	value: string;
	variant?: ChipVariant;
}) {
	return (
		<TableTooltip content={value}>
			<span className={`chip chip-${variant}`}>{value}</span>
		</TableTooltip>
	);
}

export function tableTooltipText(
	value: string | number | null | undefined,
): string {
	return value === undefined || value === null || value === ""
		? "—"
		: String(value);
}

function TableTooltip({
	children,
	content,
}: {
	children: ReactNode;
	content: string;
}) {
	return (
		<Tooltip>
			<TooltipTrigger asChild>
				<span className="tooltip-anchor" tabIndex={0}>
					{children}
				</span>
			</TooltipTrigger>
			<TooltipContent side="top" align="start" sideOffset={6}>
				{content}
			</TooltipContent>
		</Tooltip>
	);
}

function TruncatedCell({ value }: { value: string | number | null | undefined }) {
	const text = tableTooltipText(value);
	return (
		<TableTooltip content={text}>
			<span className="truncate-cell">{text}</span>
		</TableTooltip>
	);
}

// Argo/Helm badges rendered inline in the App column
function ArgoHelmBadges({ row }: { row: ResourceSummary }) {
	const badges: Array<{ label: string; cls: string }> = [];

	if (row.argoApp) {
		badges.push({ label: `Argo: ${row.argoApp}`, cls: "badge-argo" });
	}

	if (row.helmRelease) {
		badges.push({ label: `Helm: ${row.helmRelease}`, cls: "badge-helm" });
	}

	if (badges.length === 0) return null;

	return (
		<div className="row-badges">
			{badges.map((badge, i) => (
				<TableTooltip key={i} content={badge.label}>
					<span className={`badge ${badge.cls}`}>{badge.label}</span>
				</TableTooltip>
			))}
		</div>
	);
}

const columns = [
	columnHelper.accessor("name", {
		header: "Name",
		cell: (info) => <TruncatedCell value={info.getValue()} />,
	}),
	columnHelper.accessor("namespace", {
		header: "Namespace",
		cell: (info) => <TruncatedCell value={info.getValue()} />,
	}),
	columnHelper.accessor("kind", {
		header: "Kind",
		cell: (info) => <TruncatedCell value={info.getValue()} />,
	}),
	columnHelper.accessor("status", {
		header: "Status",
		cell: (info) => {
			const value = info.getValue();
			if (!value) return "—";
			const variant: ChipVariant =
				value === "Running" || value === "Succeeded" || value === "Ready"
					? "success"
					: value === "Pending" || value === "Terminating"
						? "warning"
						: value === "Failed" || value === "Error"
							? "error"
							: "neutral";
			return <StatusChip value={value} variant={variant} />;
		},
	}),
	columnHelper.accessor("ready", {
		header: "Ready",
		cell: (info) => <TruncatedCell value={info.getValue()} />,
	}),
	columnHelper.accessor("restarts", {
		header: "Restarts",
		cell: (info) => {
			const value = info.getValue();
			if (value === undefined || value === null) return "—";
			if (value === 0) return "0";
			const variant: ChipVariant =
				value > 5 ? "error" : value > 0 ? "warning" : "neutral";
			return <StatusChip value={String(value)} variant={variant} />;
		},
	}),
	columnHelper.accessor("ownerRef", {
		header: "Owner",
		cell: (info) => <TruncatedCell value={info.getValue()} />,
	}),
	columnHelper.accessor("age", {
		header: "Age",
		cell: (info) => <TruncatedCell value={info.getValue()} />,
	}),
	columnHelper.display({
		id: "argo-helm",
		header: "App",
		cell: ({ row }) => <ArgoHelmBadges row={row.original} />,
		enableSorting: false,
	}),
];

const PAGE_SIZE = 50;

function sortedRows(
	data: ResourceSummary[],
	sorting: SortingState,
): ResourceSummary[] {
	if (sorting.length === 0) return data;
	return [...data].sort((a, b) => {
		for (const { id, desc } of sorting) {
			const av = (a as unknown as Record<string, unknown>)[id];
			const bv = (b as unknown as Record<string, unknown>)[id];
			if (av == null && bv == null) continue;
			if (av == null) return desc ? 1 : -1;
			if (bv == null) return desc ? -1 : 1;
			const cmp = String(av).localeCompare(String(bv));
			if (cmp !== 0) return desc ? -cmp : cmp;
		}
		return 0;
	});
}

function filterResources(
	data: ResourceSummary[],
	search: string,
	argoAppFilter: string,
): ResourceSummary[] {
	const term = search.trim().toLowerCase();
	return data.filter((r) => {
		if (argoAppFilter && r.argoApp !== argoAppFilter) return false;
		if (!term) return true;
		return (
			r.name.toLowerCase().includes(term) ||
			r.namespace?.toLowerCase().includes(term) === true ||
			r.kind.toLowerCase().includes(term) ||
			r.ownerRef?.toLowerCase().includes(term) === true ||
			r.argoApp?.toLowerCase().includes(term) === true ||
			r.helmRelease?.toLowerCase().includes(term) === true
		);
	});
}

function uniqueArgoApps(data: ResourceSummary[]): string[] {
	return Array.from(
		new Set(
			data.map((r) => r.argoApp).filter((app): app is string => Boolean(app)),
		),
	).sort((a, b) => a.localeCompare(b));
}

export function formatResourceGroupLabel(resource: ResourceSummary): string {
	return resource.argoApp
		? `Managed by Argo app: ${resource.argoApp}`
		: "Unmanaged resources";
}

export function formatResourceTypeGroupLabel(resource: ResourceSummary): string {
	if (resource.kind.endsWith("s")) return `${resource.kind}es`;
	if (resource.kind.endsWith("y")) return `${resource.kind.slice(0, -1)}ies`;
	return `${resource.kind}s`;
}

export function resourceGroupCollapseKey(resource: ResourceSummary): string {
	return `app:${formatResourceGroupLabel(resource)}`;
}

export function resourceTypeGroupCollapseKey(resource: ResourceSummary): string {
	return `${resourceGroupCollapseKey(resource)}::type:${formatResourceTypeGroupLabel(resource)}`;
}

export function describeResourceScope(
	clusterContext: string,
	namespaces: string[],
	kinds: string[],
	argoAppFilter: string,
): ScopePill[] {
	const pills: ScopePill[] = [{ label: "Context", value: clusterContext }];
	if (namespaces.length > 0) {
		pills.push({
			label: namespaces.length === 1 ? "Namespace" : "Namespaces",
			value:
				namespaces.length <= 2
					? namespaces.join(", ")
					: `${namespaces.slice(0, 2).join(", ")} +${namespaces.length - 2}`,
		});
	}
	if (kinds.length > 0) {
		pills.push({
			label: kinds.length === 1 ? "Kind" : "Kinds",
			value:
				kinds.length <= 3
					? kinds.join(", ")
					: `${kinds.slice(0, 3).join(", ")} +${kinds.length - 3}`,
		});
	}
	if (argoAppFilter) {
		pills.push({ label: "Argo app", value: argoAppFilter });
	}
	return pills;
}

export function buildResourceHealthSummary(
	rows: ResourceSummary[],
): HealthSummary {
	return rows.reduce<HealthSummary>(
		(summary, row) => {
			const status = row.status?.toLowerCase() ?? "";
			const ready = row.ready?.toLowerCase() ?? "";
			const restarts = row.restarts ?? 0;
			const isDegraded =
				status === "failed" ||
				status === "error" ||
				status === "crashloopbackoff" ||
				status === "imagepullbackoff" ||
				ready === "false";
			const needsAttention =
				!isDegraded &&
				(status === "pending" ||
					status === "terminating" ||
					status === "unknown" ||
					restarts > 0);
			const isHealthy =
				!isDegraded &&
				!needsAttention &&
				(status === "running" ||
					status === "succeeded" ||
					status === "ready" ||
					ready === "true");

			return {
				total: summary.total + 1,
				healthy: summary.healthy + (isHealthy ? 1 : 0),
				attention: summary.attention + (needsAttention ? 1 : 0),
				degraded: summary.degraded + (isDegraded ? 1 : 0),
				restarted: summary.restarted + (restarts > 0 ? 1 : 0),
			};
		},
		{ total: 0, healthy: 0, attention: 0, degraded: 0, restarted: 0 },
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
			<div className="resource-list-state">
				<div className="skeleton-list">
					{Array.from({ length: 8 }).map((_, i) => (
						<div key={i} className="skeleton-row">
							<div className="skeleton-cell skeleton-name"></div>
							<div className="skeleton-cell skeleton-ns"></div>
							<div className="skeleton-cell skeleton-kind"></div>
							<div className="skeleton-cell skeleton-status"></div>
						</div>
					))}
				</div>
				<span className="loading-indicator">Loading resources…</span>
			</div>
		);
	}

	if (isError) {
		return (
			<div className="resource-list-state error-state">
				<span>
					Error:{" "}
					{error instanceof Error ? error.message : "Failed to load resources"}
				</span>
			</div>
		);
	}

	if (!data || data.length === 0) {
		return (
			<div className="resource-list-state empty-state">
				<span>No resources found</span>
			</div>
		);
	}

	return (
		<div className="resource-table-container">
			<div className="resource-context-strip" aria-label="Current resource scope">
				{scopePills.map((pill) => (
					<div className="scope-pill" key={pill.label}>
						<span>{pill.label}</span>
						<strong>{pill.value}</strong>
					</div>
				))}
			</div>
			<div className="resource-health-strip" aria-label="Resource health summary">
				<div className="health-card health-card-total">
					<span>Total</span>
					<strong>{healthSummary.total}</strong>
				</div>
				<div className="health-card health-card-healthy">
					<span>Healthy</span>
					<strong>{healthSummary.healthy}</strong>
				</div>
				<div className="health-card health-card-attention">
					<span>Needs attention</span>
					<strong>{healthSummary.attention}</strong>
				</div>
				<div className="health-card health-card-degraded">
					<span>Degraded</span>
					<strong>{healthSummary.degraded}</strong>
				</div>
				<div className="health-card health-card-restarted">
					<span>Restarted</span>
					<strong>{healthSummary.restarted}</strong>
				</div>
			</div>
			<div className="resource-list-toolbar">
				<input
					className="resource-search-input"
					type="text"
					placeholder="Search by name, namespace, kind, owner, Argo app, Helm release…"
					value={search}
					onChange={(e) => {
						setSearch(e.target.value);
						setPageIndex(0);
					}}
				/>
				{argoApps.length > 0 && (
					<select
						className="resource-filter-select"
						value={selectedArgoAppFilter}
						onChange={(event) => {
							onArgoAppFilterChange(event.target.value);
							setPageIndex(0);
						}}
						aria-label="Filter by Argo application"
					>
						<option value="">All Argo apps</option>
						{argoApps.map((app) => (
							<option key={app} value={app}>
								{app}
							</option>
						))}
					</select>
				)}
				{(search || selectedArgoAppFilter) && (
					<button
						className="clear-filter-btn"
						onClick={() => {
							setSearch("");
							onArgoAppFilterChange("");
							setPageIndex(0);
						}}
					>
						Clear
					</button>
				)}
				{(search || selectedArgoAppFilter) && filteredData.length === 0 && (
					<span className="filter-no-results">
						No results for current filters —{" "}
						<button
							className="clear-filter-link"
							onClick={() => {
								setSearch("");
								onArgoAppFilterChange("");
								setPageIndex(0);
							}}
						>
							clear filters
						</button>
					</span>
				)}
			</div>

			<table className="resource-table">
				<thead>
					{table.getHeaderGroups().map((headerGroup) => (
						<tr key={headerGroup.id}>
							{headerGroup.headers.map((header) => (
								<th
									key={header.id}
									onClick={header.column.getToggleSortingHandler()}
									style={{
										cursor: header.column.getCanSort() ? "pointer" : "default",
									}}
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
							<td colSpan={columns.length} className="empty-page-state">
								No resources match your filter
							</td>
						</tr>
					) : (
						table.getRowModel().rows.map((row, index) => {
							const resourceKey = `${row.original.cluster}:${row.original.kind}:${row.original.namespace ?? ""}:${row.original.name}`;
							const isSelected = selectedResourceKey === resourceKey;
							const previous =
								index > 0
									? table.getRowModel().rows[index - 1]?.original
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
										<tr className="resource-group-row">
											<td colSpan={columns.length}>
												<button
													type="button"
													className="group-toggle"
													onClick={() => toggleGroup(appCollapseKey)}
													aria-expanded={!appCollapsed}
												>
													<span className="group-chevron">
														{appCollapsed ? ">" : "v"}
													</span>
													<span>{label}</span>
													<small>
														{pageGroups.get(label) ?? 0} resources on this page
													</small>
												</button>
											</td>
										</tr>
									)}
									{showTypeGroupHeader && !appCollapsed && (
										<tr className="resource-type-group-row">
											<td colSpan={columns.length}>
												<button
													type="button"
													className="group-toggle type-toggle"
													onClick={() => toggleGroup(typeCollapseKey)}
													aria-expanded={!typeCollapsed}
												>
													<span className="group-chevron">
														{typeCollapsed ? ">" : "v"}
													</span>
													<span>{typeLabel}</span>
													<small>
														{pageTypeGroups.get(typeKey) ?? 0} on this page
													</small>
												</button>
											</td>
										</tr>
									)}
									{!hideResourceRow && (
										<tr
											className={`resource-row${isSelected ? " selected" : ""}`}
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

			<div className="table-pagination">
				<button
					className="pagination-btn"
					onClick={() => setPageIndex((p) => Math.max(0, p - 1))}
					disabled={safePageIndex === 0}
				>
					Previous
				</button>
				<span className="pagination-info">
					{totalRows} {search ? "filtered" : "total"} rows
				</span>
				<span className="pagination-page">
					Page {safePageIndex + 1} of {pageCount}
				</span>
				<button
					className="pagination-btn"
					onClick={() => setPageIndex((p) => Math.min(pageCount - 1, p + 1))}
					disabled={safePageIndex >= pageCount - 1}
				>
					Next
				</button>
			</div>
		</div>
	);
}

export const ResourceList = memo(ResourceListComponent);
