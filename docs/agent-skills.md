# Agent Skill Backlog

Candidate project-specific skills. Before installing one, define a pressure scenario, observe the work without the skill, write the smallest `SKILL.md` that fixes the observed problem, and validate it. Then install it through the normal skill path.

Read [product and architecture](product-and-architecture.md), [development workflow](development-workflow.md), [GitHub issue tracker](agents/issue-tracker.md), and the relevant ADRs before creating a domain skill. Track skill proposals and validation follow-up in GitHub Issues, and use Milestones for delivery grouping.

## First Batch

### `tauri-security-review`

Use for Tauri commands, plugins, capabilities, filesystem or shell access, Kubernetes credentials, guarded operations, and frontend/backend boundaries.

- No broad frontend shell access.
- No kubeconfig, token, certificate, or Secret leakage to frontend.
- Prefer narrow typed Tauri commands over generic bridges.
- Review capabilities and permissions when adding a plugin.
- Require an ADR for a shell plugin, broad filesystem access, a cluster-changing command, or long-lived sensitive state.

### `kube-rs-resource-api`

Use for Kubernetes list, get, watch, discovery, dynamic object, CRD, event, metrics, log, serialization, and governed-operation paths in Rust.

- Use `kube-rs` for core Kubernetes API access; do not shell out to `kubectl`.
- Keep API errors serializable and raw objects out of general frontend state.
- Follow ADR 0003 for live sessions and ADR 0004 for other cluster-changing paths.

### `k8s-ux-resource-browser`

Use for navigation, filters, tables, topology, detail panels, and browsing flows.

- Preserve context-first and namespace-first navigation.
- Keep selected scope visible and persistent global filters reliable.
- Use dense tables for repeated work.
- Keep inspection and operation surfaces separate with exact targets.

### `argocd-awareness`

Use for Argo CD detection, grouping, CRD views, connected inspection, comparison, or operations.

- Start with Kubernetes API access to Argo CRDs and tracking metadata.
- Keep Kubernetes and connected transports explicit; never silently fall back.
- Apply ADR 0013 to credentials, TLS, redaction, comparison, preflight, and allowlisted operations.
- Keep the Argo CLI, arbitrary manifests, deletes, and spec editing outside the connected path. Flux remains inspection-only unless an ADR says otherwise.

## Future Candidates

- `frontend-state-table-patterns`: add once the Svelte query, table, and local-state conventions are stable.
- `safe-k8s-mutations`: add when repeated work goes beyond the guardrails in ADRs 0003 to 0006 and ADR 0013.
- `agent-task-discipline`: keep these rules in `AGENTS.md` unless repeated drift shows that a dedicated skill helps.

Create and validate one skill before starting the next.
