# Agent Skill Backlog

This file tracks project-specific Codex skills that may be worth creating later. Do not install a skill from this list casually. First define the pressure scenario, observe what agents do without the skill, write the smallest useful `SKILL.md`, validate it, and only then install it under the normal skill path.

## First Batch

### `tauri-security-review`

Use when implementing or reviewing Tauri commands, plugins, capabilities, filesystem access, shell access, Kubernetes credential handling, guarded operations, or frontend/backend boundaries.

Rules:

- No broad shell access from the frontend.
- No kubeconfig, token, certificate, or secret leakage to the frontend.
- Prefer narrow typed Tauri commands over generic bridges.
- Review Tauri capabilities and permissions when adding plugins.
- Require an ADR for shell plugin usage, broad filesystem access, cluster-changing commands, or long-lived sensitive state.

Triggers:

- "Add a Tauri command for Kubernetes data."
- "Expose a Rust function to the frontend."
- "Add a Tauri plugin."
- "Review whether this leaks kubeconfig data."
- "Add apply/delete/scale/sync support."

### `kube-rs-resource-api`

Use when implementing Kubernetes list, get, watch, discovery, dynamic object, CRD, event, metrics, log, serialization, or governed operation paths in Rust.

Rules:

- Use `kube-rs` for normal Kubernetes API access.
- Do not shell out to `kubectl` for core list/get/watch/discovery paths.
- Keep typed resources simple while allowing discovery and dynamic resources to fit.
- Keep API errors clean and serializable.
- Keep raw Kubernetes objects out of general frontend state.
- For live-session paths, follow ADR 0003 before writing command code.
- For broader cluster-changing paths, follow ADR 0004 before writing command code.

Triggers:

- "List pods or deployments."
- "Add CRD support."
- "Implement discovery."
- "Serialize resource YAML."
- "Add watch, events, logs, or metrics support."
- "Add a guarded Kubernetes operation."

### `k8s-ux-resource-browser`

Use when changing navigation, filters, tables, topology maps, detail panels, empty states, loading states, or Kubernetes browsing flows.

Rules:

- Preserve context-first and namespace-first navigation.
- Keep selected context and namespace scope visible.
- Treat persistent global filters as core product behavior.
- Use dense, fast tables for repeated resource work.
- Keep inspection and operation surfaces visibly separate, with exact operation targets.
- Avoid marketing-page patterns inside the app shell.

Triggers:

- "Add the resource table."
- "Change the sidebar."
- "Design the detail drawer."
- "Add namespace filtering."
- "Change the topology map."

### `argocd-awareness`

Use when changing Argo CD detection, grouping, filtering, CRD views, connected inspection, resource comparison, or guarded operations.

Rules:

- Start with Kubernetes API access to Argo CD CRDs and tracking metadata.
- Detect ownership from `argocd.argoproj.io/instance`, `argocd.argoproj.io/tracking-id`, `app.kubernetes.io/instance`, and related annotations/labels.
- Keep Kubernetes and connected transports explicit; never add automatic fallback.
- Apply ADR 0013 to connected credentials, TLS, Secret redaction, comparison, preflight, and allowlisted operations.
- Keep Argo CD CLI, arbitrary manifests, deletes, and spec editing outside the connected path.
- Keep Flux inspection-only unless a focused ADR adds guarded operations.

Triggers:

- "Group resources by Argo app."
- "List Argo Applications."
- "Add ApplicationSet support."
- "Show sync and health status."

## Future Candidates

### `frontend-state-table-patterns`

Create after frontend patterns settle.

Expected scope:

- TanStack Query cache keys and loading/error conventions.
- TanStack Table row and column conventions.
- Local UI state boundaries for filters and selections.
- Rules against putting huge raw Kubernetes objects in table rows.

### `safe-k8s-mutations`

Create when repeated work expands or reviews guarded mutations beyond current ADR 0003–0006 and ADR 0013 contracts.

Expected scope:

- ADR 0004 command and UX contract.
- Server-side dry-run where available.
- Diff before apply.
- Explicit confirmation UX.
- Exact resource or provider target scope.
- Argo, Flux, and Helm ownership warnings.
- Local audit trail or event log.

### `agent-task-discipline`

Keep this in `AGENTS.md` unless repeated drift proves a dedicated skill is useful.

Expected scope:

- Inspect first.
- Plan multi-file work.
- Keep patches scoped.
- Run checks.
- Report changed files, checks, blockers, and residual risk.

## Creation Order

1. `tauri-security-review`
2. `kube-rs-resource-api`
3. `k8s-ux-resource-browser`
4. `argocd-awareness`

Create and validate one skill before starting the next.
