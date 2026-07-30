# Code Organization

App shape lives in [product-and-architecture.md](../product-and-architecture.md).

## Svelte Frontend

### `src/app/svelte/`

App shell, surfaces, settings, navigation, and cross-feature wiring. Keep product-area behavior in its feature.

### `src/features/<area>/`

Feature folders own product-area Svelte components, helpers, tests, and types. Import another feature only through its public entry point. Current areas include `argo`, `app-updates`, `command-palette`, `gitops`, `helm`, `incidents`, `live-sessions`, `rbac`, `resource-detail`, `resources`, and `workspaces`.

Create feature folder when product area has multiple components or helpers not reusable elsewhere.

### `src/components/`

Generic Svelte UI only: shared layout, generic displays, and primitives in `ui/svelte/`. Component used by one feature belongs in that feature. Component importing feature code is not generic.

### `src/lib/`

Typed Tauri wrappers, frontend-safe contracts, stores, query keys, and pure shared logic. No feature-specific UI behavior.

## Backend (`src-tauri/src/`)

`commands/` owns typed Tauri command domains. `models/` owns frontend-safe serde contracts. Keep kubeconfig contents, tokens, certificates, and unnecessary raw Kubernetes objects out of models. Add reusable Kubernetes access module only when multiple consumers need it.

## Cross-Cutting Rules

- Frontend uses typed Tauri wrappers; it never calls Kubernetes or local shell directly.
- Cluster data crosses typed Tauri boundary only.
- Live sessions follow [ADR 0003](../decisions/0003-guarded-live-sessions.md); other changing operations follow [ADR 0004](../decisions/0004-guarded-cluster-operations.md).
- Security-boundary changes require ADR.
- New top-level `src/` or `src-tauri/src/` directory requires one-line entry here first.
- `scripts/` is maintainer automation, never app runtime code.
