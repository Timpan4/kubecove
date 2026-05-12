# Milestone 1 Completion Design

## Goal

Wire scaffolded components into a working read-only Kubernetes browser with TanStack Query/Router/Table, namespace multi-select, kind filter, and read-only detail panel.

## Scope

### Frontend

- One root dashboard route: `/`
- TanStack Query for server state, TanStack Router for future routes, TanStack Table for resource grid
- Plain CSS (no Tailwind or UI kit yet)
- Left sidebar:
  - Cluster context selector (single-select dropdown)
  - Namespace multi-select (checkbox list)
  - Kind filter (checkbox list: Pod, Deployment, StatefulSet, DaemonSet, Service, Ingress, ConfigMap, Secret, PersistentVolumeClaim, Job, CronJob)
- Main area: TanStack resource table with columns: Name, Namespace, Kind, Age
- Right panel: read-only detail with "Details" and "YAML" tabs
- React component state/hooks for selected context, namespaces, kinds, and selected resource

### Backend

- Extend `list_resources` and `get_resource_details` for: StatefulSet, DaemonSet, Ingress, Secret, PersistentVolumeClaim, Job, CronJob
- Extend `get_resource_yaml` for same kinds
- No new Tauri commands

## Non-Goals

- Tailwind, shadcn/ui, or other UI kits
- Mutations, watches, logs, events
- Argo CD or Helm integration
- Persistent local state (beyond session)
- Multi-cluster simultaneous view

## Architecture

```
Frontend                     Backend (Rust/Tauri)
  |                                   |
  |-- list_kube_contexts()          --|
  |-- list_namespaces(ctx)          --|
  |-- list_resources(ctx, ns[], kind[]) --|
  |-- get_resource_details(ctx, kind, name, ns) --|
  |-- get_resource_yaml(ctx, kind, name, ns) --|
  |                                   |
  |<-- ClusterContext[]              --|
  |<-- NamespaceSummary[]            --|
  |<-- ResourceSummary[]            --|
  |<-- ResourceDetailsFull           --|
  |<-- String (YAML)                 --|
```

### Frontend Modules

```
src/
  main.tsx
  App.tsx                          # Root with router
  App.css                          # Plain CSS (all styles)
  app/
    router.tsx                     # TanStack Router: single "/" route
  lib/
    tauri.ts                       # Typed Tauri wrappers (existing)
    types.ts                       # Frontend types (existing)
    hooks.ts                       # useState/useCallback for clusterContext, selectedNamespaces, selectedKinds, selectedResource
  features/
    clusters/
      ClusterSelector.tsx          # Single-select dropdown
    namespaces/
      NamespaceList.tsx            # Multi-select checkboxes
    resources/
      ResourceTable.tsx            # TanStack Table
      KindFilter.tsx               # Kind checkbox list
    resource-detail/
      ResourceDetailPanel.tsx      # Right panel: Details + YAML tabs
```

### Backend Modules

```
src-tauri/src/
  main.rs                          # Entry point
  lib.rs                           # Tauri builder (existing)
  commands.rs                       # Tauri command handlers
    - list_kube_contexts()         (existing)
    - list_namespaces(ctx)         (existing)
    - list_resources(ctx, kind, ns) (extend for 7 new kinds)
    - get_resource_yaml(ctx, kind, name, ns) (extend for 7 new kinds)
    - get_resource_details(ctx, kind, name, ns) (extend for 7 new kinds)
  models/mod.rs                    # Serde contracts (existing)
```

## UI Layout

```
+------------------+--------------------------------+------------------+
| LEFT SIDEBAR     | MAIN CONTENT                   | RIGHT PANEL      |
| (280px fixed)    | (flex-grow)                    | (400px fixed)    |
|                  |                                |                  |
| Cluster: [dropdown] |  RESOURCE TABLE              | [Details] [YAML] |
|                  |  Name | NS | Kind | Age        |                  |
| Namespaces       |  ---- | -- | ---- | ---        | Read-only        |
| [x] kube-system  |  pod-a| ns | Pod  | 2d         | resource data    |
| [x] default      |  deploy-x| ns | Deploy | 5d   | or YAML          |
| [ ] prod        |                                |                  |
|                  |                                |                  |
| Kinds            |                                |                  |
| [x] Pod          |                                |                  |
| [x] Deployment   |                                |                  |
| [x] Service      |                                |                  |
| [ ] StatefulSet  |                                |                  |
| ...              |                                |                  |
+------------------+--------------------------------+------------------+
```

### Component Details

#### ClusterSelector
- Single `<select>` dropdown
- On change: update root component state `clusterContext`, reset `namespaces` and `selectedResource`

#### NamespaceList
- Checkbox list, not single-select click
- "Select All" / "Deselect All" header buttons
- On toggle: update root component state `selectedNamespaces`

#### KindFilter
- Checkbox list for each supported kind
- "Select All" / "Deselect All" header buttons
- On toggle: update root component state `selectedKinds`

#### ResourceTable
- TanStack Table v8
- Columns: Name, Namespace, Kind, Age
- Sortable by Name, Age
- Paginated (50 rows/page)
- Row click: set root component state `selectedResource`, open right panel

#### ResourceDetailPanel
- Tabs: "Details" and "YAML"
- Details: show metadata key/value pairs, labels, annotations, status
- YAML: show full read-only YAML from `get_resource_yaml`
- Close button: clear `selectedResource`

## Data Flow

1. User selects cluster context -> root component state `clusterContext` updated
2. TanStack Query fetches namespaces -> NamespaceList populates checkboxes
3. User selects namespaces and kinds -> root component state `selectedNamespaces`, `selectedKinds` updated
4. TanStack Query fetches resources for selected namespaces and kinds
5. ResourceTable renders results
6. User clicks row -> root component state `selectedResource` updated -> detail panel opens
7. TanStack Query fetches resource details and YAML on panel open
8. Detail panel renders Details or YAML tab

### UI State

Root dashboard component (`App.tsx`) owns these states via `useState`/`useCallback`:

```typescript
interface DashboardState {
  clusterContext: string;
  selectedNamespaces: string[];
  selectedKinds: string[];
  selectedResource: ResourceSummary | null;
}
```

State passed down via props or context to sidebar components, table, and detail panel.

## Backend Resource Coverage

| Kind              | API Version                   | Namespace-Scoped |
|-------------------|-------------------------------|------------------|
| Pod               | core/v1                       | Yes              |
| Deployment        | apps/v1                       | Yes              |
| StatefulSet       | apps/v1                       | Yes              |
| DaemonSet         | apps/v1                       | Yes              |
| Service           | core/v1                       | Yes              |
| Ingress           | networking.k8s.io/v1          | Yes              |
| ConfigMap         | core/v1                       | Yes              |
| Secret            | core/v1                       | Yes              |
| PersistentVolumeClaim | core/v1                   | Yes              |
| Job               | batch/v1                      | Yes              |
| CronJob           | batch/v1                      | Yes              |

All 11 kinds use the same pattern in `commands.rs` as existing Pod/Deployment/Service/ConfigMap.

## Error Handling

### Frontend

- TanStack Query provides `isLoading` and `isError` per query
- Each component shows loading spinner and error message with retry button
- `AppError` from backend displays as: `[kind] message`
- Empty states for: no contexts, no namespaces, no resources

### Backend

- `AppError` serde struct with `message` and `kind` fields
- `kind: "cluster"` for kube-rs errors
- `kind: "serialization"` for serde errors
- All errors return as JSON, never leak raw kubeconfig or certificate data

## Verification

- `bun run typecheck` passes
- `cargo check --manifest-path src-tauri/Cargo.toml` passes
- All existing components still render after wiring
- New kinds return results in ResourceTable
- Detail panel opens on row click and shows correct data
- Namespace multi-select filters table results
- Kind filter filters table results
