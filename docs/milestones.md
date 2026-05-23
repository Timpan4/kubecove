# Milestones

This is the goal-level progress tracker. Keep it factual: check a box only when the app behavior exists and the relevant verification has run or is clearly recorded elsewhere.

## Milestone 0: Repository Foundation

Goal: create the project container for implementation work.

- [x] Initialize git workspace.
- [x] Capture architecture constraints.
- [x] Capture milestone plan.
- [x] Capture agent guidance.
- [x] Create GitHub repository and push the foundation commit.

## Milestone 1: Read-Only Kubernetes Browser

Goal: browse common Kubernetes resources through Rust-side Tauri commands.

- [x] Scaffold Tauri v2, React, TypeScript, Bun, and Rust.
- [x] Add Bun-based frontend tooling and validation scripts.
- [x] Add Rust dependencies for Tauri commands, `kube-rs`, serde, YAML serialization, and error handling.
- [x] Implement context discovery and context-specific client construction.
- [x] Implement namespace listing.
- [x] Implement resource listing for Pods, Deployments, Services, Ingresses, ConfigMaps, Secrets, PVCs, Jobs, and CronJobs.
- [x] Implement read-only resource YAML and details.
- [x] Build sidebar, resource table, and detail panel.
- [x] Add namespace multi-select filtering.
- [x] Add read-only YAML/details tabs.
- [x] Run frontend typecheck, Rust check, and app dev mode.

## Milestone 2: Usability Pass

Goal: make the browser useful instead of merely connected.

- [x] Add resource search.
- [x] Add status chips.
- [x] Add age formatting.
- [x] Add owner reference detection.
- [x] Add Argo CD label and annotation detection.
- [x] Add Helm release label detection.
- [x] Add loading, empty, and error states.
- [x] Persist selected context and namespaces in local UI state.

## Milestone 3: Argo CD Native Read-Only Views

Goal: make Argo CD a first-class read-only grouping layer through the Kubernetes API.

- [x] Detect whether Argo CD CRDs exist in the selected context.
- [x] List Argo CD Applications.
- [x] List Argo CD ApplicationSets.
- [x] List Argo CD AppProjects.
- [x] Summarize Application sync status, health status, destination namespace, source repo, revision, and project.
- [x] Group resources by Argo application when tracking metadata exists.
- [x] Add read-only Argo application detail panel.
- [x] Add global Argo application filter.

## Milestone 4: Discovery and Grouping

Goal: broaden Kubernetes coverage without hard-coding every resource.

- [x] Add Kubernetes API discovery module.
- [x] Add CRD listing when practical.
- [x] Add dynamic-object listing for discovered resources.
- [x] Add resource kind filter backed by discovery.
- [x] Add app/owner grouping views.

## Milestone 5: Saved Workspace Launcher

Goal: restore operator intent through live workspace scopes instead of stale object state.

- [x] Add saved workspace launcher.
- [x] Add create, edit, open, and delete flows for local workspaces.
- [x] Persist workspace scope locally without kubeconfig or credential material.
- [x] Restore workspace scopes into a live curated overview.
- [x] Show unavailable saved contexts, namespaces, and kinds during restore.
- [x] Keep resource browser and Argo views reachable from workspace shortcuts.

## Milestone 6: Incident Workflow Polish

Goal: make the read-only browser useful during troubleshooting.

- [x] Add unhealthy investigation filtering.
- [x] Keep table, map, and detail selection synchronized.
- [x] Add compact incident summaries to resource details.
- [x] Add incident triage timelines to resource details.
- [x] Prioritize warning events and preserve pod log container selection while details refresh.
- [x] Add workspace overview shortcuts for unhealthy, warning, and restarted resources.

## Milestone 7: Stabilization and Release Readiness

Goal: make the current read-only incident workflow a reliable beta baseline.

- [x] Repair frontend verification after local `node_modules` corruption.
- [x] Run frontend typecheck, frontend tests, and Rust tests.
- [x] Audit read-only logs, events, topology, and watch refresh behavior against the current UI.
- [x] Reconcile README and milestone tracking for already-built incident surfaces.
- [ ] Run a manual Tauri smoke test against a readable cluster before the next beta release.

## Cross-Cutting Tracks

### Security and Safety

- [x] Verify frontend cannot invoke arbitrary shell commands.
- [x] Keep kubeconfig secrets Rust-side.
- [x] Keep the current product read-only.
- [ ] Make future mutation commands explicit and permission-gated.
- [ ] Add ADR before Argo CD API, CLI, sync, rollback, or diff support.

### Agent Skills

Tracked in detail in [agent-skills.md](agent-skills.md). Create in this order:

- [ ] `tauri-security-review`
- [ ] `kube-rs-resource-api`
- [ ] `k8s-ux-resource-browser`
- [ ] `argocd-awareness`
- [ ] Revisit `frontend-state-table-patterns` after frontend patterns settle.
- [ ] Revisit `safe-k8s-mutations` before mutation work starts.

## Later Product Areas

- YAML edit/apply with explicit guardrails
- port-forward
- pod exec
- richer Helm workflows
- deeper RBAC and security inspection
- customizable workspace layout
- custom density modes
- adaptive defaults for explicit workspace types
- context-aware AI assistance
- durable local saved state
