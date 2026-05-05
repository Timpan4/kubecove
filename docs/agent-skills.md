# Agent Skill Backlog

This file captures project-specific skill ideas before they become installed Codex skills.

Actual skills should not be created casually. Before turning one of these into a real skill, run a small skill-writing pass: define pressure scenarios, check what agents do without the skill, write the minimal `SKILL.md`, validate it, and only then install it under the user's normal skill path.

## First Batch To Create

### `tauri-security-review`

Use when implementing or reviewing Tauri commands, capabilities, plugins, filesystem access, shell access, Kubernetes credential handling, or frontend/backend boundaries.

Core rules:

- No broad shell access from the frontend.
- No kubeconfig, token, certificate, or secret leakage to React.
- Prefer narrow typed Tauri commands over generic command bridges.
- Review Tauri capabilities and permissions when adding plugins.
- Require an ADR for shell plugin usage, broad filesystem access, mutation commands, or long-lived sensitive state.

Good trigger examples:

- "Add a Tauri command for Kubernetes data."
- "Expose a Rust function to React."
- "Add a Tauri plugin."
- "Review whether this leaks kubeconfig data."

### `kube-rs-resource-api`

Use when implementing Kubernetes list, get, watch, discovery, dynamic object, CRD, or serialization paths in Rust.

Core rules:

- Use `kube-rs` for normal Kubernetes API access.
- Do not shell out to `kubectl` for list/get/watch/discovery in the core data path.
- Keep typed resource support simple now, but structure code so dynamic resources and CRDs fit later.
- Keep API errors clean and serializable.
- Keep raw Kubernetes objects out of the frontend unless a read-only detail/YAML view explicitly asks for them.

Good trigger examples:

- "List pods or deployments."
- "Add CRD support."
- "Implement discovery."
- "Serialize resource YAML."

### `k8s-ux-resource-browser`

Use when designing or changing the Kubernetes browsing UI, navigation, filters, tables, detail panels, empty states, or loading/error states.

Core rules:

- Preserve context-first and namespace-first navigation.
- Keep selected cluster/context and namespaces obvious at all times.
- Treat persistent global filters as core product behavior.
- Use dense, fast tables for repeated resource work.
- Put raw YAML/details in a read-only detail panel for the first milestones.
- Avoid landing-page or marketing-style UI patterns inside the app.

Good trigger examples:

- "Add the resource table."
- "Change the sidebar."
- "Design the detail drawer."
- "Add namespace filtering."

### `argocd-awareness`

Use when adding Argo CD detection, grouping, filtering, Application views, ApplicationSet views, AppProject views, or future Argo CD API/CLI behavior.

Core rules:

- Start with Kubernetes API access to Argo CD CRDs and tracking metadata.
- Detect ownership from `argocd.argoproj.io/instance`, `argocd.argoproj.io/tracking-id`, `app.kubernetes.io/instance`, and related annotations/labels.
- Treat Argo CD API, CLI, sync, rollback, and diff as future features that need explicit ADRs.
- Warn before future mutations against Argo-managed resources.
- Keep first Argo support read-only.

Good trigger examples:

- "Group resources by Argo app."
- "List Argo Applications."
- "Add ApplicationSet support."
- "Show sync and health status."

## Future Candidates

### `frontend-state-table-patterns`

Create after the frontend scaffold has real patterns.

Expected scope:

- TanStack Query cache keys and error/loading conventions.
- TanStack Table row shape and column conventions.
- Local UI state boundaries for filters and selections.
- Rules against putting huge raw Kubernetes objects in table rows.

### `safe-k8s-mutations`

Create before implementing apply, delete, scale, restart, sync, rollback, port-forward, exec, or terminal-backed actions.

Expected scope:

- Read-only default.
- Server-side dry-run where available.
- Diff before apply.
- Explicit confirmation UX.
- Argo/Helm ownership warnings.
- Clear audit trail in local UI state or logs.

### `agent-task-discipline`

Probably keep this in `AGENTS.md` unless repeated agent drift proves a dedicated skill is useful.

Expected scope:

- Inspect first.
- Make a small plan for multi-file work.
- Keep patches minimal.
- Run checks.
- Report changed files, checks, blockers, and residual risks.

## Creation Order

1. `tauri-security-review`
2. `kube-rs-resource-api`
3. `k8s-ux-resource-browser`
4. `argocd-awareness`

Create one skill at a time. Validate each before starting the next.
