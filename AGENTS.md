# Agent Guide

This repository is for a local desktop Kubernetes IDE built with Tauri v2, React, TypeScript, Rust, and `kube-rs`.

Before making implementation changes, read:

- `docs/architecture-blueprint.md`
- `docs/milestones.md`
- `docs/todos.md`

## Core Constraints

- Keep the MVP read-only.
- Do not deploy anything into Kubernetes clusters.
- Do not expose raw kubeconfig contents to the frontend.
- Do not let frontend code run arbitrary shell commands.
- Use `kube-rs` and the Kubernetes API for core list/get/discovery flows.
- Treat `kubectl`, Helm, and Argo CD CLIs as future optional sidecars or fallbacks, not as the core data path.
- Keep modules small and typed so future agent work can target focused files.

## Preferred Architecture

Rust-side Tauri commands own Kubernetes access:

- `list_kube_contexts`
- `list_namespaces`
- `list_resources`
- `get_resource_yaml`

The React app should call only typed Tauri command wrappers. Kubernetes credentials and kubeconfig parsing belong in Rust modules under `src-tauri/src`.

## Implementation Style

- Prefer boring, stable dependencies.
- Keep changes reviewable and scoped.
- Add tests or checks where risk justifies them.
- Use clear type names that match the docs unless there is a good reason to change them.
- Add comments only for non-obvious architecture decisions.
- Add TODOs for future features without implementing them early.

## Git Hygiene

- Use small commits.
- Avoid unrelated formatting churn.
- Do not rewrite user changes.
- Prefer branch names with the `codex/` prefix for agent-created feature branches.
