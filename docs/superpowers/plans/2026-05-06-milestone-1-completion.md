# Milestone 1 Completion Implementation Plan

Historical implementation plan from 2026-05-06. This plan has been completed and superseded by the current codebase. Keep it as context for why Milestone 1 took its original shape; do not copy commands or snippets from this file into new work without checking current source.

## Goal

Build the first useful read-only Kubernetes browser:

- context selector
- namespace multi-select
- resource kind filter
- resource table
- read-only detail panel
- read-only YAML
- Rust-side Kubernetes access through Tauri commands

## Constraints

- Read-only MVP.
- No raw kubeconfig data in React.
- No arbitrary frontend shell execution.
- Use `kube-rs` for Kubernetes API access.
- Use typed Tauri wrappers from the frontend.
- Keep changes scoped and verifiable.

## Planned File Areas

Frontend:

- `src/main.tsx` for app providers
- `src/app/router.tsx` for the initial route
- `src/App.tsx` and `src/App.css` for the first dashboard shell
- `src/lib/tauri.ts`, `src/lib/types.ts`, and local state hooks
- cluster, namespace, resource table, kind filter, and detail panel components

Backend:

- Tauri command handlers for contexts, namespaces, resources, YAML, and details
- serde models for frontend-safe cluster, namespace, resource, detail, and error payloads

Package management:

- TanStack Query, Router, and Table added through Bun
- lockfile committed with dependency changes

## Task Flow

1. Add TanStack packages and provider/router setup.
2. Extend backend common resource coverage.
3. Add typed frontend wrappers, constants, and local dashboard state.
4. Build the three-column dashboard layout.
5. Build namespace and kind filters.
6. Build the resource table.
7. Build the read-only detail/YAML panel.
8. Run verification and update milestone tracking.

## Resource Kinds

The milestone targeted:

- Pod
- Deployment
- StatefulSet
- DaemonSet
- Service
- Ingress
- ConfigMap
- Secret
- PersistentVolumeClaim
- Job
- CronJob

Later milestones added discovery-backed resource kinds and broader views.

## Verification

Original checks:

```sh
bun run typecheck
cargo check --manifest-path src-tauri/Cargo.toml
```

Manual behavior checks:

1. Context dropdown shows local kubeconfig contexts.
2. Selecting a context loads namespaces.
3. Selecting namespaces and kinds loads table rows.
4. Selecting a row opens the detail panel.
5. Details and YAML tabs are read-only.
6. Namespace and kind filters affect results.
7. All targeted common kinds return results when present in the selected cluster.

## Outcome

Milestone 1 completed the initial browser. Current work should use:

- [Architecture Blueprint](../../architecture-blueprint.md) for current structure.
- [Development Workflow](../../development-workflow.md) for current commands.
- [Milestones](../../milestones.md) for current status.
- [Engineering Handbook](../../handbook/) for organization and size rules.
