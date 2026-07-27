# Code Organization

This page is the placement rulebook. The app shape lives in [architecture-blueprint.md](../architecture-blueprint.md).

## Frontend (`src/`)

### `src/features/<area>/`

Feature folders hold product-area-specific components, hooks, helpers, and types. Other features should import only public exports from the feature entry point, not deep private helpers.

Current areas:

- `app-updates/`
- `argo/`
- `helm/`
- `live-sessions/`
- `rbac/`
- `resource-detail/`
- `resources/`
- `settings/`
- `workspaces/`

Create a new feature folder when a product area has more than one component or hook and the code is not reusable elsewhere.

### `src/components/`

Use only for generic, feature-agnostic UI:

- shadcn primitives in `ui/`
- shared layout chrome
- generic badges, timestamp components, skeletons, and metadata display helpers

A component used by one feature belongs in that feature folder. A component that imports from a feature folder is not generic.

Some older flat components still live here (`ClusterSelector`, `NamespaceList`, `KindList`, `SidebarTree`). Move them into feature folders when touched for structural work; do not add new feature-specific components here.

### `src/lib/`

Pure logic only. No JSX.

Current responsibilities:

- `tauri.ts` - typed Tauri command wrappers
- `types.ts` - frontend mirrors of serde contracts
- `hooks.ts` - Zustand stores and composed dashboard hooks
- `settings.ts` - settings persistence
- `workspaces.ts` - workspace scope and restore helpers
- `queryKeys.ts` - query key helpers
- `tree-nav.ts` - sidebar tree helpers
- `resource-visuals.ts`, `resource-health.ts`, `resource-metrics.ts` - pure resource presentation logic
- `diagnostics.ts`, `usage-metrics.ts`, `app-version.ts`, `release-channel.ts`, `utils.ts` - shared utilities

Logic used by one feature belongs inside that feature.

### `src/app/`

App-level wiring and frames only: router, top bar, app shell helpers, shared loading frames, and cross-feature app hooks.

## Backend (`src-tauri/src/`)

### `commands/`

One file or folder per command domain. Each `#[tauri::command]` belongs in its domain:

- `contexts`
- `namespaces`
- `resources`
- `events`
- `discovery`
- `streams`
- `metrics`
- `argo`
- `helm`
- `rbac`
- `sessions`
- `usage`

Shared command helpers belong in `commands/helpers.rs` or a helper submodule. `commands/mod.rs` re-exports commands for `lib.rs` invoke registration.

### `models/`

Serde contracts shared with the frontend. Split by domain when a model set grows:

- `cluster`
- `namespace`
- `resource`
- `discovery`
- `events`
- `streams`
- `metrics`
- `argo`
- `helm`
- `rbac`
- `usage`
- `error`

Models must be frontend-safe. They must not contain raw kubeconfig contents, tokens, certificates, or unnecessary full Kubernetes objects.

### Future `kube/`

Reserve `src-tauri/src/kube/` for reusable Kubernetes access logic once commands are no longer the only backend consumer. Do not create it just to move code around.

## New Top-Level Directories

Adding a new top-level directory under `src/` or `src-tauri/src/` requires a one-line entry in this file before the directory is created.

Repo-level `scripts/` contains maintainer automation. It must not contain app runtime code.

## Cross-Cutting Rules

- Frontend never calls Kubernetes directly.
- Frontend never executes shell commands.
- All cluster data crosses the Tauri boundary through typed wrappers.
- Kubeconfig contents, tokens, and certificate data never cross into the frontend.
- Live-session commands follow [ADR 0003](../decisions/0003-guarded-live-sessions.md).
- Other cluster-changing commands follow [ADR 0004](../decisions/0004-guarded-cluster-operations.md).
- Security-sensitive changes require an ADR.
