# Milestone 1 Completion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire scaffolded components into a working read-only Kubernetes browser with TanStack Query/Router/Table, namespace multi-select, kind filter, and read-only detail panel.

**Architecture:** Frontend React/TypeScript app mounted at `/` uses TanStack Query to fetch cluster/namespaces/resources, root component state via useState/useCallback for clusterContext/selectedNamespaces/selectedKinds/selectedResource, plain CSS for layout, and a 280px sidebar + flex-grow main + 400px right panel three-column layout. Backend Rust/Tauri commands.rs extended for 7 new kinds (StatefulSet, DaemonSet, Ingress, Secret, PersistentVolumeClaim, Job, CronJob) in list_resources/get_resource_yaml/get_resource_details. All Kubernetes access stays Rust-side; no raw kubeconfig reaches frontend.

**Tech Stack:** Tauri v2, React 18, TypeScript, Bun, TanStack Query v5, TanStack Router v2, TanStack Table v8, plain CSS, kube-rs, k8s-openapi.

---

## File Structure

### Frontend (src/)

- Modify: `src/main.tsx` — add QueryClientProvider and Router
- Modify: `src/App.tsx` — replace scaffold with root dashboard layout and state hooks
- Modify: `src/App.css` — replace scaffold CSS with three-column layout CSS
- Create: `src/app/router.tsx` — TanStack Router with single `/` route
- Create: `src/lib/hooks.ts` — useState/useCallback for clusterContext, selectedNamespaces, selectedKinds, selectedResource
- Modify: `src/lib/types.ts` — add SUPPORTED_KINDS constant array
- Create: `src/features/clusters/ClusterSelector.tsx` — single-select dropdown
- Modify: `src/features/clusters/ClusterSelector.tsx` — wire into App state
- Create: `src/features/namespaces/NamespaceList.tsx` — multi-select checkbox list with Select All/Deselect All
- Create: `src/features/resources/KindFilter.tsx` — kind checkbox list with Select All/Deselect All
- Modify: `src/features/resources/ResourceList.tsx` — replace plain table with TanStack Table
- Create: `src/features/resources/ResourceTable.tsx` — TanStack Table with Name/NS/Kind/Age columns, sortable, paginated 50/page
- Create: `src/features/resource-detail/ResourceDetailPanel.tsx` — right panel with Details + YAML tabs

### Backend (src-tauri/src/)

- Modify: `src-tauri/src/commands.rs` — add StatefulSet, DaemonSet, Ingress, Secret, PersistentVolumeClaim, Job, CronJob to resources_summary_from, resource_yaml_from, and resource_details_from

### Package Management

- Modify: `package.json` — add TanStack packages (described in Task 1)

---

## Task 1: Add TanStack Dependencies and Provider/Router Setup

**Files:**
- Modify: `package.json`
- Create: `src/app/router.tsx`
- Modify: `src/main.tsx`

- [ ] **Step 1: Review existing package.json**

Read `package.json` to understand current dependencies and scripts.

- [ ] **Step 2: Add TanStack packages**

Run:
```
bun add @tanstack/react-query @tanstack/react-router @tanstack/react-table
```

Expected: `package.json` updated with new deps; `bun.lock` updated.

- [ ] **Step 3: Create router file**

Create `src/app/router.tsx`:

```typescript
import { createRouter as createTanStackRouter } from "@tanstack/react-router";
import { RootRoute } from "@tanstack/react-router";
import { Dashboard } from "../features/dashboard/Dashboard";

const rootRoute = new RootRoute();

const indexRoute = rootRoute.addChildren([
  {
    path: "/",
    component: Dashboard,
  },
]);

export const router = createTanStackRouter({ routeTree: rootRoute });
```

- [ ] **Step 4: Update main.tsx**

Modify `src/main.tsx` to wrap App with QueryClientProvider and RouterProvider:

```typescript
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { RouterProvider } from "@tanstack/react-router";
import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { router } from "./app/router";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 30_000,
      retry: 2,
    },
  },
});

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router}>
        <App />
      </RouterProvider>
    </QueryClientProvider>
  </React.StrictMode>
);
```

- [ ] **Step 5: Run typecheck**

Run: `bun run typecheck`
Expected: No TypeScript errors.

---

## Task 2: Extend Rust Resource Coverage for 7 New Kinds

**Files:**
- Modify: `src-tauri/src/commands.rs:142-223` — add StatefulSet, DaemonSet, Ingress, Secret, PersistentVolumeClaim, Job, CronJob to resources_summary_from match
- Modify: `src-tauri/src/commands.rs:254-272` — add same 7 kinds to resource_yaml_from match
- Modify: `src-tauri/src/commands.rs:302-383` — add same 7 kinds to resource_details_from match

- [ ] **Step 1: Add StatefulSet to resources_summary_from**

In `resources_summary_from`, add new match arm after "ConfigMap" block:

```rust
"StatefulSet" => {
    let api: Api<k8s_openapi::api::apps::v1::StatefulSet> = if let Some(ns) = &namespace {
        Api::namespaced(client, ns)
    } else {
        Api::all(client)
    };
    let statefulsets = api.list(&Default::default()).await.map_err(|e| AppError::kube(e.to_string()))?;
    statefulsets
        .iter()
        .map(|ss| ResourceSummary {
            kind: "StatefulSet".to_string(),
            cluster: cluster_context.clone(),
            name: ss.metadata.name.clone().unwrap_or_default(),
            namespace: ss.metadata.namespace.clone(),
            age: resource_age(ss.metadata.creation_timestamp.clone().map(|t| {
                Utc.timestamp_opt(t.0.as_second() as i64, 0).single().unwrap_or_else(Utc::now)
            })),
        })
        .collect()
}
```

- [ ] **Step 2: Add DaemonSet to resources_summary_from**

Add match arm for "DaemonSet" using `k8s_openapi::api::apps::v1::DaemonSet`.

- [ ] **Step 3: Add Ingress to resources_summary_from**

Add match arm for "Ingress" using `k8s_openapi::api::networking::v1::Ingress`.

- [ ] **Step 4: Add Secret to resources_summary_from**

Add match arm for "Secret" using `k8s_openapi::api::core::v1::Secret`.

- [ ] **Step 5: Add PersistentVolumeClaim to resources_summary_from**

Add match arm for "PersistentVolumeClaim" using `k8s_openapi::api::core::v1::PersistentVolumeClaim`.

- [ ] **Step 6: Add Job to resources_summary_from**

Add match arm for "Job" using `k8s_openapi::api::batch::v1::Job`.

- [ ] **Step 7: Add CronJob to resources_summary_from**

Add match arm for "CronJob" using `k8s_openapi::api::batch::v1::CronJob`. Note: CronJob in k8s-openapi uses `batch/v1` with `CronJob` type name (not `CronJob` vs `batch/v1beta1` — use `batch/v1` stable version).

- [ ] **Step 8: Update resource_yaml_from match arms**

Add same 7 arms to `resource_yaml_from` (lines 254-272), using same k8s types as above, each calling `fetch_and_serialize`. Example for StatefulSet:
```rust
"StatefulSet" => {
    let (_ss, yaml) = fetch_and_serialize::<k8s_openapi::api::apps::v1::StatefulSet>(client, namespace.as_deref(), &name).await?;
    Ok(yaml)
}
```

- [ ] **Step 9: Update resource_details_from match arms**

Add same 7 arms to `resource_details_from` (lines 302-383), using same k8s types as above. Each arm builds ResourceDetailsFull with:
- metadata from serde_json::to_value(&resource.metadata)
- status from resource.status.as_ref().map(|s| serde_json::to_value(s).ok()).flatten() (None for Secret/PVC which have no meaningful status field)
- summary from resource metadata

- [ ] **Step 10: Run cargo check**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: Compiles without errors. If missing k8s-openapi types, run `cargo add k8s-openapi` in src-tauri directory to add latest version.

---

## Task 3: Update Typed Frontend Wrappers/Types/Constants

**Files:**
- Modify: `src/lib/types.ts` — add SUPPORTED_KINDS constant
- Modify: `src/lib/tauri.ts` — if needed for any new wrapper patterns
- Create: `src/lib/hooks.ts` — dashboard state hooks

- [ ] **Step 1: Add SUPPORTED_KINDS constant to types.ts**

Add at end of `src/lib/types.ts`:

```typescript
export const SUPPORTED_KINDS = [
  "Pod",
  "Deployment",
  "StatefulSet",
  "DaemonSet",
  "Service",
  "Ingress",
  "ConfigMap",
  "Secret",
  "PersistentVolumeClaim",
  "Job",
  "CronJob",
] as const;

export type SupportedKind = typeof SUPPORTED_KINDS[number];
```

- [ ] **Step 2: Create hooks.ts with dashboard state**

Create `src/lib/hooks.ts`:

```typescript
import { useState, useCallback } from "react";
import type { ResourceSummary } from "./types";

export function useDashboardState() {
  const [clusterContext, setClusterContext] = useState<string>("");
  const [selectedNamespaces, setSelectedNamespaces] = useState<string[]>([]);
  const [selectedKinds, setSelectedKinds] = useState<string[]>([]);
  const [selectedResource, setSelectedResource] = useState<ResourceSummary | null>(null);

  const resetNamespaces = useCallback(() => {
    setSelectedNamespaces([]);
    setSelectedResource(null);
  }, []);

  const resetAll = useCallback(() => {
    setClusterContext("");
    setSelectedNamespaces([]);
    setSelectedKinds([]);
    setSelectedResource(null);
  }, []);

  return {
    clusterContext,
    setClusterContext,
    selectedNamespaces,
    setSelectedNamespaces,
    selectedKinds,
    setSelectedKinds,
    selectedResource,
    setSelectedResource,
    resetNamespaces,
    resetAll,
  };
}
```

- [ ] **Step 3: Run typecheck**

Run: `bun run typecheck`
Expected: No TypeScript errors.

---

## Task 4: Build Dashboard Layout with Plain CSS

**Files:**
- Modify: `src/App.css` — replace with three-column layout CSS
- Modify: `src/App.tsx` — root dashboard component with sidebar and main layout

- [ ] **Step 1: Write App.css three-column layout**

Replace contents of `src/App.css`:

```css
/* Reset and base */
*, *::before, *::after { box-sizing: border-box; margin: 0; padding: 0; }

:root {
  --color-bg: #0f1117;
  --color-surface: #1a1d27;
  --color-border: #2a2d3a;
  --color-text: #e4e4e7;
  --color-text-muted: #71717a;
  --color-accent: #3b82f6;
  --color-accent-hover: #2563eb;
  --sidebar-width: 280px;
  --panel-width: 400px;
  --font-sans: system-ui, -apple-system, sans-serif;
}

html, body, #root {
  height: 100%;
  background: var(--color-bg);
  color: var(--color-text);
  font-family: var(--font-sans);
  font-size: 14px;
}

/* Layout */
.root {
  display: flex;
  height: 100vh;
  overflow: hidden;
}

/* Left Sidebar */
.sidebar {
  width: var(--sidebar-width);
  min-width: var(--sidebar-width);
  background: var(--color-surface);
  border-right: 1px solid var(--color-border);
  display: flex;
  flex-direction: column;
  overflow-y: auto;
}

.sidebar-section {
  padding: 12px 16px;
  border-bottom: 1px solid var(--color-border);
}

.sidebar-section-title {
  font-size: 11px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--color-text-muted);
  margin-bottom: 8px;
}

/* Main Content */
.main-content {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.main-header {
  padding: 12px 20px;
  border-bottom: 1px solid var(--color-border);
  display: flex;
  align-items: center;
  gap: 12px;
}

.main-header h1 {
  font-size: 16px;
  font-weight: 600;
}

.main-body {
  flex: 1;
  overflow: auto;
  padding: 16px 20px;
}

/* Right Panel */
.right-panel {
  width: var(--panel-width);
  min-width: var(--panel-width);
  background: var(--color-surface);
  border-left: 1px solid var(--color-border);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.panel-header {
  padding: 12px 16px;
  border-bottom: 1px solid var(--color-border);
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.panel-header-title {
  font-size: 14px;
  font-weight: 600;
}

.panel-tabs {
  display: flex;
  border-bottom: 1px solid var(--color-border);
}

.panel-tab {
  padding: 8px 16px;
  font-size: 13px;
  cursor: pointer;
  border-bottom: 2px solid transparent;
  color: var(--color-text-muted);
  background: none;
  border-top: none;
  border-left: none;
  border-right: none;
}

.panel-tab.active {
  color: var(--color-accent);
  border-bottom-color: var(--color-accent);
}

.panel-body {
  flex: 1;
  overflow: auto;
  padding: 16px;
}

/* Form elements */
select, input[type="text"] {
  background: var(--color-bg);
  border: 1px solid var(--color-border);
  color: var(--color-text);
  padding: 6px 10px;
  border-radius: 4px;
  font-size: 13px;
  width: 100%;
}

select { cursor: pointer; }

/* Checkbox list */
.checkbox-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.checkbox-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 4px 0;
  cursor: pointer;
  font-size: 13px;
}

.checkbox-item input[type="checkbox"] {
  width: 14px;
  height: 14px;
  accent-color: var(--color-accent);
}

.list-header {
  display: flex;
  justify-content: space-between;
  margin-bottom: 6px;
}

.list-header-btn {
  font-size: 11px;
  color: var(--color-accent);
  cursor: pointer;
  background: none;
  border: none;
  padding: 0;
}

.list-header-btn:hover { text-decoration: underline; }

/* Table */
.resource-table {
  width: 100%;
  border-collapse: collapse;
}

.resource-table th {
  text-align: left;
  padding: 8px 12px;
  font-size: 11px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--color-text-muted);
  border-bottom: 1px solid var(--color-border);
  cursor: pointer;
  user-select: none;
}

.resource-table th:hover { color: var(--color-text); }

.resource-table td {
  padding: 10px 12px;
  border-bottom: 1px solid var(--color-border);
  font-size: 13px;
}

.resource-table tr:hover td {
  background: rgba(255,255,255,0.03);
  cursor: pointer;
}

.resource-table tr.selected td {
  background: rgba(59, 130, 246, 0.1);
}

/* Pagination */
.table-pagination {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 0;
  font-size: 13px;
  color: var(--color-text-muted);
}

.pagination-controls {
  display: flex;
  gap: 4px;
}

.pagination-btn {
  padding: 4px 8px;
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  color: var(--color-text);
  border-radius: 4px;
  cursor: pointer;
  font-size: 12px;
}

.pagination-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.pagination-btn.active {
  background: var(--color-accent);
  border-color: var(--color-accent);
}

/* Loading / Error / Empty states */
.loading-state, .error-state, .empty-state {
  padding: 40px 20px;
  text-align: center;
  color: var(--color-text-muted);
}

.error-state { color: #ef4444; }

.loading-spinner {
  width: 24px;
  height: 24px;
  border: 2px solid var(--color-border);
  border-top-color: var(--color-accent);
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
  margin: 0 auto 12px;
}

@keyframes spin { to { transform: rotate(360deg); } }

/* YAML display */
.yaml-block {
  font-family: "Menlo", "Consolas", monospace;
  font-size: 12px;
  white-space: pre-wrap;
  word-break: break-word;
  color: var(--color-text);
  line-height: 1.5;
}

/* Detail key-value */
.detail-row {
  display: flex;
  gap: 12px;
  padding: 6px 0;
  border-bottom: 1px solid var(--color-border);
}

.detail-key {
  font-weight: 500;
  color: var(--color-text-muted);
  min-width: 120px;
}

.detail-value {
  color: var(--color-text);
  word-break: break-word;
}

/* Utility */
.visually-hidden {
  position: absolute;
  width: 1px;
  height: 1px;
  padding: 0;
  margin: -1px;
  overflow: hidden;
  clip: rect(0, 0, 0, 0);
  border: 0;
}
```

- [ ] **Step 2: Create App.tsx dashboard skeleton**

Modify `src/App.tsx` to be root dashboard layout with sidebar and main area structure. Actual component wiring happens in Task 5-7:

```typescript
import "./App.css";
import { useDashboardState } from "./lib/hooks";
import { ClusterSelector } from "./features/clusters/ClusterSelector";
import { NamespaceList } from "./features/namespaces/NamespaceList";
import { KindFilter } from "./features/resources/KindFilter";
import { ResourceTable } from "./features/resources/ResourceTable";
import { ResourceDetailPanel } from "./features/resource-detail/ResourceDetailPanel";

function App() {
  const {
    clusterContext,
    setClusterContext,
    selectedNamespaces,
    setSelectedNamespaces,
    selectedKinds,
    setSelectedKinds,
    selectedResource,
    setSelectedResource,
  } = useDashboardState();

  return (
    <div className="root">
      <aside className="sidebar">
        <div className="sidebar-section">
          <ClusterSelector
            value={clusterContext}
            onChange={setClusterContext}
          />
        </div>
        <div className="sidebar-section">
          <NamespaceList
            clusterContext={clusterContext}
            selectedNamespaces={selectedNamespaces}
            onChange={setSelectedNamespaces}
          />
        </div>
        <div className="sidebar-section">
          <KindFilter
            selectedKinds={selectedKinds}
            onChange={setSelectedKinds}
          />
        </div>
      </aside>
      <div className="main-content">
        <ResourceTable
          clusterContext={clusterContext}
          selectedNamespaces={selectedNamespaces}
          selectedKinds={selectedKinds}
          selectedResource={selectedResource}
          onSelectResource={setSelectedResource}
        />
      </div>
      {selectedResource && (
        <ResourceDetailPanel
          resource={selectedResource}
          onClose={() => setSelectedResource(null)}
        />
      )}
    </div>
  );
}

export default App;
```

---

## Task 5: Build Namespace Multi-Select and Kind Filter

**Files:**
- Modify: `src/features/namespaces/NamespaceList.tsx` — existing single-select; replace with multi-select checkbox list
- Create: `src/features/resources/KindFilter.tsx` — kind checkbox list

- [ ] **Step 1: Replace NamespaceList.tsx with multi-select version**

Read `src/features/namespaces/NamespaceList.tsx`. Replace content with:

```typescript
import { useQuery } from "@tanstack/react-query";
import { useEffect } from "react";
import { listNamespaces } from "../../lib/tauri";
import { createTauriClient } from "../../lib/tauri";
import type { NamespaceSummary } from "../../lib/types";

interface NamespaceListProps {
  clusterContext: string;
  selectedNamespaces: string[];
  onChange: (namespaces: string[]) => void;
}

export function NamespaceList({ clusterContext, selectedNamespaces, onChange }: NamespaceListProps) {
  const client = createTauriClient();

  const { data: namespaces, isLoading, isError, error } = useQuery({
    queryKey: ["namespaces", clusterContext],
    queryFn: () => listNamespaces(client, clusterContext),
    enabled: !!clusterContext,
  });

  // Reset selections when cluster changes
  useEffect(() => {
    onChange([]);
  }, [clusterContext]);

  const toggleNamespace = (name: string) => {
    if (selectedNamespaces.includes(name)) {
      onChange(selectedNamespaces.filter((n) => n !== name));
    } else {
      onChange([...selectedNamespaces, name]);
    }
  };

  const selectAll = () => {
    if (namespaces) {
      onChange(namespaces.map((ns: NamespaceSummary) => ns.name));
    }
  };

  const deselectAll = () => {
    onChange([]);
  };

  if (!clusterContext) {
    return (
      <div className="sidebar-section">
        <div className="sidebar-section-title">Namespaces</div>
        <div className="empty-state" style={{ padding: "16px 0", fontSize: "12px" }}>
          Select a cluster first
        </div>
      </div>
    );
  }

  return (
    <div className="sidebar-section">
      <div className="sidebar-section-title">Namespaces</div>
      {isLoading && (
        <div className="loading-state" style={{ padding: "16px 0" }}>
          <div className="loading-spinner" style={{ width: "16px", height: "16px", marginBottom: "8px" }}></div>
          <span style={{ fontSize: "12px" }}>Loading...</span>
        </div>
      )}
      {isError && (
        <div className="error-state" style={{ padding: "16px 0", fontSize: "12px" }}>
          Error: {(error as Error).message}
        </div>
      )}
      {namespaces && (
        <>
          <div className="list-header">
            <button className="list-header-btn" onClick={selectAll}>
              Select All
            </button>
            <button className="list-header-btn" onClick={deselectAll}>
              Deselect All
            </button>
          </div>
          <div className="checkbox-list">
            {namespaces.map((ns: NamespaceSummary) => (
              <label key={ns.name} className="checkbox-item">
                <input
                  type="checkbox"
                  checked={selectedNamespaces.includes(ns.name)}
                  onChange={() => toggleNamespace(ns.name)}
                />
                <span>{ns.name}</span>
              </label>
            ))}
          </div>
        </>
      )}
    </div>
  );
}
```

- [ ] **Step 2: Create KindFilter.tsx**

Create `src/features/resources/KindFilter.tsx`:

```typescript
import { SUPPORTED_KINDS } from "../../lib/types";

interface KindFilterProps {
  selectedKinds: string[];
  onChange: (kinds: string[]) => void;
}

export function KindFilter({ selectedKinds, onChange }: KindFilterProps) {
  const toggleKind = (kind: string) => {
    if (selectedKinds.includes(kind)) {
      onChange(selectedKinds.filter((k) => k !== kind));
    } else {
      onChange([...selectedKinds, kind]);
    }
  };

  const selectAll = () => {
    onChange([...SUPPORTED_KINDS]);
  };

  const deselectAll = () => {
    onChange([]);
  };

  return (
    <div className="sidebar-section">
      <div className="sidebar-section-title">Resource Kinds</div>
      <div className="list-header">
        <button className="list-header-btn" onClick={selectAll}>
          Select All
        </button>
        <button className="list-header-btn" onClick={deselectAll}>
          Deselect All
        </button>
      </div>
      <div className="checkbox-list">
        {SUPPORTED_KINDS.map((kind) => (
          <label key={kind} className="checkbox-item">
            <input
              type="checkbox"
              checked={selectedKinds.includes(kind)}
              onChange={() => toggleKind(kind)}
            />
            <span>{kind}</span>
          </label>
        ))}
      </div>
    </div>
  );
}
```

- [ ] **Step 3: Run typecheck**

Run: `bun run typecheck`
Expected: No TypeScript errors.

---

## Task 6: Build TanStack Resource Table

**Files:**
- Modify: `src/features/resources/ResourceList.tsx` — remove old plain table
- Create: `src/features/resources/ResourceTable.tsx` — TanStack Table v8

- [ ] **Step 1: Create ResourceTable.tsx**

Create `src/features/resources/ResourceTable.tsx`:

```typescript
import { useQuery } from "@tanstack/react-query";
import { useMemo, useState } from "react";
import {
  useReactTable,
  getCoreRowModel,
  getSortedRowModel,
  getPaginationRowModel,
  flexRender,
  createColumnHelper,
  SortingState,
} from "@tanstack/react-table";
import { listResources } from "../../lib/tauri";
import { createTauriClient } from "../../lib/tauri";
import type { ResourceSummary } from "../../lib/types";

const columnHelper = createColumnHelper<ResourceSummary>();

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
  columnHelper.accessor("age", {
    header: "Age",
    cell: (info) => info.getValue(),
  }),
];

interface ResourceTableProps {
  clusterContext: string;
  selectedNamespaces: string[];
  selectedKinds: string[];
  selectedResource: ResourceSummary | null;
  onSelectResource: (resource: ResourceSummary | null) => void;
}

export function ResourceTable({
  clusterContext,
  selectedNamespaces,
  selectedKinds,
  selectedResource,
  onSelectResource,
}: ResourceTableProps) {
  const [sorting, setSorting] = useState<SortingState>([]);
  const client = createTauriClient();

  // Fetch resources for all selected namespaces and kinds
  // Use separate queries per namespace-kind pair, then combine
  const enabled = !!clusterContext && selectedNamespaces.length > 0 && selectedKinds.length > 0;

  const { data: allResources = [], isLoading, isError, error, isFetching } = useQuery({
    queryKey: ["resources", clusterContext, selectedNamespaces, selectedKinds],
    queryFn: async () => {
      const results: ResourceSummary[] = [];
      for (const ns of selectedNamespaces) {
        for (const kind of selectedKinds) {
          try {
            const resources = await listResources(client, clusterContext, kind, ns);
            results.push(...resources);
          catch (err) {
            // Skip failed namespace-kind combinations
            console.warn(`Failed to fetch ${kind} in ${ns}:`, err);
          }
        }
      }
      return results;
    },
    enabled,
  });

  const table = useReactTable({
    data: allResources,
    columns,
    state: { sorting },
    onSortingChange: setSorting,
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
    getPaginationRowModel: getPaginationRowModel(),
    initialState: { pagination: { pageSize: 50 } },
  });

  if (!enabled) {
    return (
      <div className="main-content">
        <div className="main-header">
          <h1>Resources</h1>
        </div>
        <div className="empty-state">
          <p>Select a cluster, namespace(s), and kind(s) to view resources.</p>
        </div>
      </div>
    );
  }

  return (
    <div className="main-content">
      <div className="main-header">
        <h1>Resources</h1>
        {isFetching && (
          <span style={{ fontSize: "12px", color: "var(--color-text-muted)" }}>
            Refreshing...
          </span>
        )}
      </div>
      <div className="main-body">
        {isLoading && (
          <div className="loading-state">
            <div className="loading-spinner"></div>
            <p>Loading resources...</p>
          </div>
        )}
        {isError && (
          <div className="error-state">
            <p>Error loading resources: {(error as Error).message}</p>
          </div>
        )}
        {!isLoading && allResources.length === 0 && (
          <div className="empty-state">
            <p>No resources found for the selected criteria.</p>
          </div>
        )}
        {!isLoading && allResources.length > 0 && (
          <>
            <table className="resource-table">
              <thead>
                {table.getHeaderGroups().map((headerGroup) => (
                  <tr key={headerGroup.id}>
                    {headerGroup.headers.map((header) => (
                      <th
                        key={header.id}
                        onClick={header.column.getToggleSortingHandler()}
                      >
                        {flexRender(header.column.columnDef.header, header.getContext())}
                        {{
                          asc: " ↑",
                          desc: " ↓",
                        }[header.column.getIsSorted() as string] ?? null}
                      </th>
                    ))}
                  </tr>
                ))}
              </thead>
              <tbody>
                {table.getRowModel().rows.map((row) => {
                  const isSelected =
                    selectedResource?.name === row.original.name &&
                    selectedResource?.namespace === row.original.namespace &&
                    selectedResource?.kind === row.original.kind;
                  return (
                    <tr
                      key={row.id}
                      className={isSelected ? "selected" : ""}
                      onClick={() =>
                        onSelectResource(
                          isSelected ? null : row.original
                        )
                      }
                    >
                      {row.getVisibleCells().map((cell) => (
                        <td key={cell.id}>
                          {flexRender(cell.column.columnDef.cell, cell.getContext())}
                        </td>
                      ))}
                    </tr>
                  );
                })}
              </tbody>
            </table>
            <div className="table-pagination">
              <span>
                {table.getFilteredRowModel().rows.length} total resources
              </span>
              <div className="pagination-controls">
                <button
                  className="pagination-btn"
                  onClick={() => table.previousPage()}
                  disabled={!table.getCanPreviousPage()}
                >
                  Previous
                </button>
                <button
                  className="pagination-btn"
                  onClick={() => table.nextPage()}
                  disabled={!table.getCanNextPage()}
                >
                  Next
                </button>
              </div>
            </div>
          </>
        )}
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Run typecheck**

Run: `bun run typecheck`
Expected: No TypeScript errors.

---

## Task 7: Build Read-Only Detail Panel with Details and YAML Tabs

**Files:**
- Create: `src/features/resource-detail/ResourceDetailPanel.tsx`

- [ ] **Step 1: Create ResourceDetailPanel.tsx**

Create `src/features/resource-detail/ResourceDetailPanel.tsx`:

```typescript
import { useQuery } from "@tanstack/react-query";
import { useState } from "react";
import { getResourceDetails, getResourceYaml } from "../../lib/tauri";
import { createTauriClient } from "../../lib/tauri";
import type { ResourceSummary } from "../../lib/types";

interface ResourceDetailPanelProps {
  resource: ResourceSummary;
  onClose: () => void;
}

type Tab = "details" | "yaml";

export function ResourceDetailPanel({ resource, onClose }: ResourceDetailPanelProps) {
  const [activeTab, setActiveTab] = useState<Tab>("details");
  const client = createTauriClient();

  const { data: details, isLoading: detailsLoading } = useQuery({
    queryKey: ["resource-details", resource.cluster, resource.kind, resource.name, resource.namespace],
    queryFn: () =>
      getResourceDetails(client, resource.cluster, resource.kind, resource.name, resource.namespace ?? undefined),
  });

  const { data: yaml, isLoading: yamlLoading } = useQuery({
    queryKey: ["resource-yaml", resource.cluster, resource.kind, resource.name, resource.namespace],
    queryFn: () =>
      getResourceYaml(client, resource.cluster, resource.kind, resource.name, resource.namespace ?? undefined),
    enabled: activeTab === "yaml",
  });

  const formatMetadata = (metadata: Record<string, unknown>): Array<{ key: string; value: unknown }> => {
    const entries: Array<{ key: string; value: unknown }> = [];
    if (metadata.name) entries.push({ key: "Name", value: metadata.name });
    if (metadata.namespace) entries.push({ key: "Namespace", value: metadata.namespace });
    if (metadata.uid) entries.push({ key: "UID", value: metadata.uid });
    if (metadata.resourceVersion) entries.push({ key: "Resource Version", value: metadata.resourceVersion });
    if (metadata.creationTimestamp) entries.push({ key: "Created", value: metadata.creationTimestamp });
    if (metadata.labels) entries.push({ key: "Labels", value: JSON.stringify(metadata.labels, null, 2) });
    if (metadata.annotations) entries.push({ key: "Annotations", value: JSON.stringify(metadata.annotations, null, 2) });
    return entries;
  };

  return (
    <div className="right-panel">
      <div className="panel-header">
        <span className="panel-header-title">
          {resource.name}
        </span>
        <button
          onClick={onClose}
          style={{
            background: "none",
            border: "none",
            color: "var(--color-text-muted)",
            cursor: "pointer",
            fontSize: "18px",
            padding: "0 4px",
          }}
          aria-label="Close panel"
        >
          ×
        </button>
      </div>
      <div className="panel-tabs">
        <button
          className={`panel-tab ${activeTab === "details" ? "active" : ""}`}
          onClick={() => setActiveTab("details")}
        >
          Details
        </button>
        <button
          className={`panel-tab ${activeTab === "yaml" ? "active" : ""}`}
          onClick={() => setActiveTab("yaml")}
        >
          YAML
        </button>
      </div>
      <div className="panel-body">
        {activeTab === "details" && (
          <>
            {detailsLoading && (
              <div className="loading-state">
                <div className="loading-spinner" style={{ width: "16px", height: "16px", marginBottom: "8px" }}></div>
                <span style={{ fontSize: "12px" }}>Loading...</span>
              </div>
            )}
            {!detailsLoading && details && (
              <>
                <div style={{ marginBottom: "16px", fontSize: "12px", color: "var(--color-text-muted)" }}>
                  {details.summary.kind} in {details.summary.namespace ?? "cluster-scoped"}
                </div>
                {details.status && (
                  <div style={{ marginBottom: "16px" }}>
                    <div style={{ fontSize: "12px", fontWeight: 600, marginBottom: "8px", color: "var(--color-text-muted)" }}>
                      Status
                    </div>
                    <pre style={{ fontSize: "12px", whiteSpace: "pre-wrap" }}>
                      {JSON.stringify(details.status, null, 2)}
                    </pre>
                  </div>
                )}
                <div>
                  <div style={{ fontSize: "12px", fontWeight: 600, marginBottom: "8px", color: "var(--color-text-muted)" }}>
                    Metadata
                  </div>
                  {formatMetadata(details.metadata as Record<string, unknown>).map(({ key, value }) => (
                    <div key={key} className="detail-row">
                      <span className="detail-key">{key}</span>
                      <span className="detail-value">
                        {typeof value === "string" ? value : JSON.stringify(value)}
                      </span>
                    </div>
                  ))}
                </div>
              </>
            )}
          </>
        )}
        {activeTab === "yaml" && (
          <>
            {yamlLoading && (
              <div className="loading-state">
                <div className="loading-spinner" style={{ width: "16px", height: "16px", marginBottom: "8px" }}></div>
                <span style={{ fontSize: "12px" }}>Loading YAML...</span>
              </div>
            )}
            {!yamlLoading && yaml && (
              <pre className="yaml-block">{yaml}</pre>
            )}
          </>
        )}
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Run typecheck**

Run: `bun run typecheck`
Expected: No TypeScript errors.

---

## Task 8: Verification and Milestone/Todo Checkbox Updates

**Files:**
- Run verification commands
- Check `docs/milestones.md` and `docs/todos.md` for Milestone 1 items to mark complete

- [ ] **Step 1: Run cargo check on backend**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: Compiles without errors or warnings.

- [ ] **Step 2: Run frontend typecheck**

Run: `bun run typecheck`
Expected: No TypeScript errors.

- [ ] **Step 3: Run lint (if available)**

Run: `bun run lint` (or equivalent lint command from package.json)
Expected: No lint errors.

- [ ] **Step 4: Read milestones and todos**

Read `docs/milestones.md` and `docs/todos.md` to identify which items belong to Milestone 1 completion. List them here.

- [ ] **Step 5: Manual smoke test (describe expected behavior)**

After implementation, the app should:
1. Show cluster dropdown in sidebar with all kubeconfig contexts
2. Selecting a cluster populates namespace checkboxes
3. Selecting namespaces + kinds populates the resource table
4. Clicking a table row opens the right detail panel with Details and YAML tabs
5. Details tab shows metadata key/values; YAML tab shows full resource YAML
6. Namespace multi-select filters table to show resources from selected namespaces only
7. Kind filter filters table to show only selected kinds
8. All 11 supported kinds (Pod, Deployment, StatefulSet, DaemonSet, Service, Ingress, ConfigMap, Secret, PersistentVolumeClaim, Job, CronJob) appear in the kind filter and return results when selected

---

## Self-Review Checklist

- [ ] No TBD/TODO/placeholders anywhere in the plan
- [ ] All spec requirements mapped to tasks: TanStack Query/Router/Table, namespace multi-select, kind filter, detail panel with Details+YAML tabs, 7 new Rust kinds, plain CSS layout
- [ ] File paths are exact and match existing project structure
- [ ] Commands are exact (bun add, bun run typecheck, cargo check)
- [ ] No commit steps (user did not ask to commit)
- [ ] No source implementation files modified (all are Modify/Create operations for new files or clearly scoped replacements)
- [ ] TDD steps included for each task with typecheck/cargo check as verification
- [ ] Constraints preserved: read-only MVP, no raw kubeconfig frontend, no frontend shell execution, typed Tauri wrappers only
- [ ] Tech stack respected: TanStack packages, plain CSS, no Tailwind/shadcn/Zustand introduced