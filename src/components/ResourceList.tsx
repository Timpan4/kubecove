import { useEffect, useMemo, useState } from "react";
import {
  useQuery,
} from "@tanstack/react-query";
import {
  createColumnHelper,
  flexRender,
  getCoreRowModel,
  useReactTable,
  type SortingState,
} from "@tanstack/react-table";
import { createTauriClient, listResources } from "../lib/tauri";
import type { ResourceSummary } from "../lib/types";

interface ResourceListProps {
  clusterContext: string;
  selectedNamespaces: string[];
  selectedKinds: string[];
  selectedResource: ResourceSummary | null;
  onResourceSelect: (resource: ResourceSummary) => void;
}

interface FetchKey {
  kind: string;
  namespace: string;
}

function buildFetchKeys(namespaces: string[], kinds: string[]): FetchKey[] {
  const keys: FetchKey[] = [];
  for (const ns of namespaces) {
    for (const k of kinds) {
      keys.push({ kind: k, namespace: ns });
    }
  }
  return keys;
}

async function fetchResourcePage(
  clusterContext: string,
  fetchKeys: FetchKey[]
): Promise<ResourceSummary[]> {
  const client = createTauriClient();
  const results = await Promise.all(
    fetchKeys.map(({ kind, namespace }) =>
      listResources(client, clusterContext, kind, namespace)
    )
  );
  return results.flat();
}

const columnHelper = createColumnHelper<ResourceSummary>();

type ChipVariant = "neutral" | "success" | "warning" | "error" | "info";

function StatusChip({ value, variant = "neutral" }: { value: string; variant?: ChipVariant }) {
  return (
    <span className={`chip chip-${variant}`}>
      {value}
    </span>
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
        <span key={i} className={`badge ${badge.cls}`}>
          {badge.label}
        </span>
      ))}
    </div>
  );
}

const columns = [
  columnHelper.accessor("name", {
    header: "Name",
    cell: (info) => info.getValue(),
  }),
  columnHelper.accessor("namespace", {
    header: "Namespace",
    cell: (info) => info.getValue() ?? "—",
  }),
  columnHelper.accessor("kind", {
    header: "Kind",
    cell: (info) => info.getValue(),
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
    cell: (info) => info.getValue() ?? "—",
  }),
  columnHelper.accessor("restarts", {
    header: "Restarts",
    cell: (info) => {
      const value = info.getValue();
      if (value === undefined || value === null) return "—";
      if (value === 0) return "0";
      const variant: ChipVariant = value > 5 ? "error" : value > 0 ? "warning" : "neutral";
      return <StatusChip value={String(value)} variant={variant} />;
    },
  }),
  columnHelper.accessor("ownerRef", {
    header: "Owner",
    cell: (info) => info.getValue() ?? "—",
  }),
  columnHelper.accessor("age", {
    header: "Age",
    cell: (info) => info.getValue(),
  }),
  columnHelper.display({
    id: "argo-helm",
    header: "App",
    cell: ({ row }) => <ArgoHelmBadges row={row.original} />,
    enableSorting: false,
  }),
];

const PAGE_SIZE = 50;

function sortedRows(data: ResourceSummary[], sorting: SortingState): ResourceSummary[] {
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

function filterResources(data: ResourceSummary[], search: string): ResourceSummary[] {
  if (!search.trim()) return data;
  const term = search.toLowerCase();
  return data.filter(
    (r) =>
      r.name.toLowerCase().includes(term) ||
      r.namespace?.toLowerCase().includes(term) === true ||
      r.kind.toLowerCase().includes(term) ||
      r.ownerRef?.toLowerCase().includes(term) === true ||
      r.argoApp?.toLowerCase().includes(term) === true ||
      r.helmRelease?.toLowerCase().includes(term) === true
  );
}

export function ResourceList({
  clusterContext,
  selectedNamespaces,
  selectedKinds,
  selectedResource,
  onResourceSelect,
}: ResourceListProps) {
  const [pageIndex, setPageIndex] = useState(0);
  const [sorting, setSorting] = useState<SortingState>([]);
  const [search, setSearch] = useState("");

  // Stable query key: serialize fetchKeys as sorted JSON string
  const fetchKeys = useMemo(
    () => buildFetchKeys(selectedNamespaces, selectedKinds),
    [selectedNamespaces.join(","), selectedKinds.join(",")]
  );
  const queryKey = useMemo(
    () => ["resources", clusterContext, ...fetchKeys.map((k) => `${k.kind}:${k.namespace}`)] as const,
    [clusterContext, fetchKeys]
  );

  // Reset page index when filters change
  useEffect(() => {
    setPageIndex(0);
  }, [clusterContext, selectedNamespaces.join(","), selectedKinds.join(",")]);

  const { data, isPending, isError, error } = useQuery({
    queryKey,
    queryFn: () => fetchResourcePage(clusterContext, fetchKeys),
    enabled: fetchKeys.length > 0,
    staleTime: 30_000,
  });

  const filteredData = useMemo(() => filterResources(data ?? [], search), [data, search]);
  const sortedData = useMemo(() => sortedRows(filteredData, sorting), [filteredData, sorting]);

  const totalRows = sortedData.length;
  const pageCount = Math.max(1, Math.ceil(totalRows / PAGE_SIZE));

  // Clamp pageIndex to valid range
  const safePageIndex = Math.min(pageIndex, Math.max(0, pageCount - 1));
  const startRow = safePageIndex * PAGE_SIZE;
  const endRow = startRow + PAGE_SIZE;
  const pageRows = sortedData.slice(startRow, endRow);

  const table = useReactTable({
    data: pageRows,
    columns,
    state: { sorting },
    onSortingChange: setSorting,
    getCoreRowModel: getCoreRowModel(),
  });

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
        <span>Error: {error instanceof Error ? error.message : "Failed to load resources"}</span>
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
        {search && (
          <button
            className="clear-filter-btn"
            onClick={() => {
              setSearch("");
              setPageIndex(0);
            }}
          >
            Clear
          </button>
        )}
        {search && filteredData.length === 0 && (
          <span className="filter-no-results">
            No results for "{search}" —{" "}
            <button
              className="clear-filter-link"
              onClick={() => {
                setSearch("");
                setPageIndex(0);
              }}
            >
              clear filter
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
                  style={{ cursor: header.column.getCanSort() ? "pointer" : "default" }}
                >
                  {flexRender(header.column.columnDef.header, header.getContext())}
                  {header.column.getIsSorted() === "asc" ? " ↑" : header.column.getIsSorted() === "desc" ? " ↓" : ""}
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
            table.getRowModel().rows.map((row) => {
              const isSelected =
                selectedResource !== null &&
                row.original.name === selectedResource.name &&
                row.original.namespace === selectedResource.namespace &&
                row.original.kind === selectedResource.kind;
              return (
                <tr
                  key={row.id}
                  className={`resource-row${isSelected ? " selected" : ""}`}
                  onClick={() => onResourceSelect(row.original)}
                >
                  {row.getVisibleCells().map((cell) => (
                    <td key={cell.id}>
                      {flexRender(cell.column.columnDef.cell, cell.getContext())}
                    </td>
                  ))}
                </tr>
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