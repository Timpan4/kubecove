# Code Organization

Where things go. The target shape lives in [architecture-blueprint.md](../architecture-blueprint.md); this page is the *rules*.

## Frontend (`src/`)

### `src/features/<area>/`

Anything specific to one product area. A feature folder is **self-contained**: its components, hooks, helpers, and types live inside it. Other features import only the feature's public surface (named exports from a top-level file, not deep paths into private helpers).

A new feature folder is created when:
- A product area gains more than one component or hook of its own, AND
- That code is not reusable outside the area.

Current areas: `argo/`, `resource-detail/`, `resources/`, `settings/`, `workspaces/`. Future candidates: `clusters/`, `namespaces/`.

### `src/components/`

Only **generic, feature-agnostic** reusables. Examples that belong here: `ui/` (shadcn primitives), layout shells, badges, formatted timestamps, generic skeletons.

A component **does not** belong here if it's used by exactly one feature. Move it into that feature's folder.

### `src/lib/`

Pure logic: utilities, types, the typed Tauri wrapper, the Zustand store. **No JSX.** No feature-specific logic.

Current entries:
- `tauri.ts` — typed wrappers around Tauri commands.
- `types.ts` — frontend copies of the serde contracts.
- `hooks.ts` — Zustand stores and the composed `useDashboardState` hook.
- `settings.ts` — settings persistence helpers.
- `tree-nav.ts` — sidebar tree navigation helpers (pure).
- `resource-visuals.ts` — pure status → visual mapping.
- `diagnostics.ts` — diagnostic logging.
- `utils.ts` — tiny shared helpers.

### `src/app/`

App-level wiring only: router, providers. One or two files.

## Backend (`src-tauri/src/`)

### `commands/`

One file per command domain. Each `#[tauri::command]` function lives in the file matching its domain. Shared helpers (`list_params`, `base_resource_summary`, `fetch_and_serialize`, age/timestamp formatting) live in `commands/helpers.rs`. The `mod.rs` re-exports the commands so `lib.rs`'s invoke handler registration imports from one place.

Target domains (introduce as commands accumulate): `contexts`, `namespaces`, `resources`, `events`, `discovery`, `argo`.

### `models/`

Serde contracts shared with the frontend. When `models/mod.rs` exceeds the soft cap, split by domain (`cluster.rs`, `namespace.rs`, `resource.rs`, `argo.rs`, `error.rs`). Until then, one file is fine.

### `kube/` (does not yet exist)

Reserved for the day Kubernetes access has a second consumer beyond `commands/` (e.g. watch streams, log streaming). YAGNI until that consumer appears.

## New top-level directories

Adding a new top-level directory under `src/` or `src-tauri/src/` requires a one-line entry in this file describing what belongs there. Update the handbook **before** creating the directory, not after.

## Cross-cutting rules

- Frontend never calls Kubernetes directly. All cluster data flows through typed Tauri wrappers in `lib/tauri.ts`.
- Frontend never executes shell commands. Period.
- Kubeconfig contents, tokens, and certificate data never cross the Tauri boundary. Rust redacts before serializing.
- Security-sensitive changes require an ADR (see [AGENTS.md](../../AGENTS.md) for the list).
