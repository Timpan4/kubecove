# Agent Guide

This repository is for a local desktop Kubernetes IDE built with Tauri v2, React, TypeScript, Bun, Rust, and `kube-rs`.

Before making implementation changes, read:

- `docs/architecture-blueprint.md`
- `docs/product-vision.md`
- `docs/milestones.md`
- `docs/todos.md`
- `docs/product-inspiration.md`
- `docs/agent-skills.md`
- `docs/development-workflow.md`

## Core Constraints

- Keep the MVP read-only.
- Do not deploy anything into Kubernetes clusters.
- Do not expose raw kubeconfig contents to the frontend.
- Do not let frontend code run arbitrary shell commands.
- Use `kube-rs` and the Kubernetes API for core list/get/discovery flows.
- Treat `kubectl`, Helm, and Argo CD CLIs as future optional sidecars or fallbacks, not as the core data path.
- Treat Argo CD as a native product area, starting with Kubernetes API access to Argo CD CRDs and tracking metadata.
- Keep modules small and typed so future agent work can target focused files.

## Preferred Architecture

Rust-side Tauri commands own Kubernetes access:

- `list_kube_contexts`
- `list_namespaces`
- `list_resources`
- `get_resource_yaml`

The React app should call only typed Tauri command wrappers. Kubernetes credentials and kubeconfig parsing belong in Rust modules under `src-tauri/src`.

## Implementation Style

- Prefer stable, well-maintained libraries with strong ecosystem support. Avoid niche or experimental dependencies unless they clearly solve a project-specific problem.
- Use Bun for frontend package management, scripts, and app scaffolding unless a tool requires a different runner.
- Commit `bun.lock` when frontend dependencies exist.
- Do not introduce duplicate router, table, state, styling, or command-wrapper libraries without an ADR.
- Keep changes reviewable and scoped.
- Add tests or checks where risk justifies them.
- Use clear type names that match the docs unless there is a good reason to change them.
- Add comments only for non-obvious architecture decisions.
- Add TODOs for future features without implementing them early.

## Tauri Command Contracts

- All frontend Kubernetes data must go through typed wrappers in `src/lib/tauri.ts`.
- Every new Tauri command needs a Rust serde model, a TypeScript type, and a typed frontend wrapper.
- Command errors must serialize into user-visible application errors.
- Do not add broad command payloads that leak kubeconfig, tokens, certificate data, or arbitrary filesystem contents.

## Security-Sensitive Changes

Write an ADR before changing:

- the frontend/backend security boundary
- the JavaScript runtime or package manager
- the Kubernetes access path
- Tauri plugin permissions or capabilities
- mutation support for Kubernetes objects
- Argo CD API, CLI, sync, rollback, or diff integration
- long-lived local persistence of cluster-derived data

## Product Inspiration

- Use K8Studio as a public feature benchmark for Kubernetes IDE capabilities.
- Use Aptakube as a public benchmark for clean, low-friction Kubernetes UX.
- Do not copy K8Studio or Aptakube code, assets, branding, proprietary layouts, or marketing text.
- Preserve this app's differentiator: context-first and namespace-first workflows.

## Verification

- Enable repo hooks with `git config core.hooksPath .githooks`.
- Run the relevant checks before claiming work is complete.
- Frontend changes should pass `bun run typecheck`; run `bun run lint` once linting exists.
- Rust backend changes should pass `cargo check --manifest-path src-tauri/Cargo.toml`.
- If a check cannot run, state the exact blocker and what remains unverified.
- When completing scoped work, check off the corresponding items in `docs/milestones.md` and `docs/todos.md`.

## Skill Backlog

- Use `docs/agent-skills.md` as the source of truth for project-specific skill ideas.
- Do not create or install actual Codex skills from that backlog without a focused skill-writing and validation pass.
- Until those skills exist, apply the documented rules manually when working in matching areas.

## Git Hygiene

- Use small commits.
- Avoid unrelated formatting churn.
- Do not rewrite user changes.
- Prefer branch names with the `codex/` prefix for agent-created feature branches.
