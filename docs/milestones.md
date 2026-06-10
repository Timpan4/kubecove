# Milestones

This is the goal-level progress tracker. Keep it factual: check a box only when the app behavior exists and the relevant verification has run or is clearly recorded elsewhere.

## Milestone 0: Repository Foundation

Goal: create the project container for implementation work.

- [x] Initialize git workspace.
- [x] Capture architecture constraints.
- [x] Capture milestone plan.
- [x] Capture agent guidance.
- [x] Create GitHub repository and push the foundation commit.

## Milestone 1: Kubernetes Inspection Browser

Goal: browse common Kubernetes resources through Rust-side Tauri commands.

- [x] Scaffold Tauri v2, React, TypeScript, Bun, and Rust.
- [x] Add Bun-based frontend tooling and validation scripts.
- [x] Add Rust dependencies for Tauri commands, `kube-rs`, serde, YAML serialization, and error handling.
- [x] Implement context discovery and context-specific client construction.
- [x] Implement namespace listing.
- [x] Implement resource listing for Pods, Deployments, Services, Ingresses, ConfigMaps, Secrets, PVCs, Jobs, and CronJobs.
- [x] Implement resource YAML and detail inspection.
- [x] Build sidebar, resource table, and detail panel.
- [x] Add namespace multi-select filtering.
- [x] Add YAML and details tabs.
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

## Milestone 3: Argo CD Native Inspection Views

Goal: make Argo CD a first-class grouping layer through the Kubernetes API.

- [x] Detect whether Argo CD CRDs exist in the selected context.
- [x] List Argo CD Applications.
- [x] List Argo CD ApplicationSets.
- [x] List Argo CD AppProjects.
- [x] Summarize Application sync status, health status, destination namespace, source repo, revision, and project.
- [x] Group resources by Argo application when tracking metadata exists.
- [x] Add Argo application detail panel.
- [x] Add global Argo application filter.

## Milestone 3A: GitOps Provider Expansion

Goal: expand the Argo CD product surface into GitOps while keeping provider integrations Kubernetes-API-first and inspection-only.

- [x] Rename the navigation surface to GitOps while preserving existing Argo CD command names and saved state.
- [x] Add ADR 0007 for GitOps providers as Kubernetes-API-first and inspection-only by default.
- [x] Add Flux detection, list, and detail commands for Source, Kustomize, Helm, Notification, and Image API CRDs.
- [x] Add Flux tables and detail panels for provider-specific inspection.
- [x] Add GitOps ownership metadata to resource summaries while preserving legacy Argo and Helm metadata.
- [x] Add GitOps resource filtering for Argo CD Applications, Flux Kustomizations, and Flux HelmReleases.
- [ ] Run manual Flux smoke testing against a readable cluster with Flux installed.

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

Goal: make the inspection browser useful during troubleshooting.

- [x] Add unhealthy investigation filtering.
- [x] Keep table, map, and detail selection synchronized.
- [x] Add compact incident summaries to resource details.
- [x] Add incident triage timelines to resource details.
- [x] Prioritize warning events and preserve pod log container selection while details refresh.
- [x] Add workspace overview shortcuts for unhealthy, warning, and restarted resources.

## Milestone 7: Stabilization and Release Readiness

Goal: make the current inspection-first incident workflow a reliable beta baseline.

- [x] Repair frontend verification after local `node_modules` corruption.
- [x] Run frontend typecheck, frontend tests, and Rust tests.
- [x] Audit logs, events, topology, and watch refresh behavior against the current UI.
- [x] Reconcile README and milestone tracking for already-built incident surfaces.
- [ ] Run a manual Tauri smoke test against a readable cluster before the next beta release.

## 0.3.0 Smoke Test Findings - 2026-05-26

Test target: local Tauri dev app backed by the `admin@solid-k8s` context after KubeCove 0.3.0 was released. The installed Windows app was also launched, but deeper click-through used the dev window because it was easier to foreground and capture.

- [x] Pass: workspace launcher loaded saved workspace `admin@solid-k8s` and listed namespaces without exposing kubeconfig contents.
- [x] Pass: opening the saved workspace restored the overview for all namespaces and showed live cluster counts, incident shortcuts, Argo CD summary, and CPU/memory footer.
- [x] Pass: resource browser opened from the overview.
- [ ] Issue: Pod resource listing remained on `Loading all namespaces` for more than 20 seconds, blocking table, detail, YAML, event, log, metrics, and topology smoke coverage in this run.
- [x] Fixed: KubeCove dev mode uses ports 1430/1431 so it can run alongside another Tauri project on the default 1420/1421 ports.
- [x] Fixed in 0.4.3: the Resources shortcut opens the saved workspace resource scope directly instead of an empty browser state.
- [x] Fixed in 0.4.3: sidebar group disclosure chevrons now select the same row they expand so selection and expansion feel aligned.
- [x] Fixed in 0.4.3: workspace card keyboard order reaches Open before Edit and Delete.
- [x] Documented in 0.4.3: Windows smoke automation needs the Tauri window explicitly foregrounded; the WebView exposes only an opaque `WRY_WEBVIEW` pane through UI Automation.

## 0.4.3 Release-Hardening Checks - 2026-06-06

Target: patch beta 0.4.3 using the existing readable `admin@solid-k8s` context before publishing.

- [x] Version metadata bumped to 0.4.3 in frontend, Tauri, Rust, and lockfile metadata.
- [x] README and release docs describe guarded exact-Pod exec as a shipped feature and keep apply/delete/scale/sync/rollback out of release claims.
- [x] Overview Resources shortcut remains pinned to the saved workspace scope with regression coverage.
- [x] Sidebar chevron clicks select the same node they expand with regression coverage.
- [x] Workspace card action order keeps Open before Edit and Delete with regression coverage.
- [x] Windows smoke guidance records that the Tauri window must be foregrounded before click-through because the WebView is exposed as an opaque `WRY_WEBVIEW` pane.
- [x] Run local automated baseline: `bun run typecheck`, `bun test`, `bun run rust:test`, and `bun run rust:check`.
- [ ] Rerun `bun run release:dry-run` after the 0.4.3 bump reaches `origin/main`; the 2026-06-06 pre-merge dry run correctly failed because `origin/main` still reports 0.4.2, equal to latest tag `app-v0.4.2`.
- [x] Run partial manual Tauri smoke on `admin@solid-k8s`: launcher context and namespace loading, workspace restore, overview resource scope, resource browser, topology, metrics, and selected-resource details/events.
- [x] Confirm saved workspace resource listing no longer remains stuck on `Loading all namespaces`; backend `list_resource_scope` completed with 293-313 rows in 315-642 ms during the 2026-06-06 smoke.
- [ ] Complete manual Pod detail/YAML/logs and guarded Pod exec click-through from exact Pod detail before claiming 0.4.3 release readiness; Windows WebView capture became opaque during this pass.
- [ ] Rerun port-forward manager smoke before publishing 0.4.3.

## 0.5.0 Release-Hardening Checks - 2026-06-08

Target: beta 0.5.0 after selected-resource YAML apply and Helm reconciliation reached `origin/main`.

- [x] Version metadata bumped to 0.5.0 in frontend, Tauri, Rust, and lockfile metadata.
- [x] README and release docs describe selected-resource YAML apply as shipped while keeping broad apply/delete/scale/sync/rollback out of release claims.
- [x] Changelog records selected-resource YAML apply, Helm reconciliation, ownership flow animation, and WebView process-tree fixes.
- [x] Run local automated baseline: `bun run typecheck`, `bun test`, `bun run rust:test`, and `bun run rust:check`.
- [ ] Run `bun run release:dry-run` after the 0.5.0 bump reaches `origin/main`.
- [ ] Run manual Tauri smoke on `admin@solid-k8s`: workspace restore, resource browser, Pod details/YAML/logs, guarded Pod exec, selected-resource YAML apply, Helm reconciliation, and port-forward manager.

## Milestone 8: Guarded Live Sessions

Goal: introduce live operational sessions without turning KubeCove into a shell wrapper.

- [x] Add ADR 0003 for guarded live Kubernetes sessions.
- [x] Add Pod and selector-backed Service port-forward command, models, typed frontend wrappers, and visible session controls.
- [x] Add workspace Port Forwards manager with saved Service presets and restore prompts.
- [x] Run a manual port-forward smoke test against a readable cluster before release.
- [x] Add a separate design pass before pod exec.

## Milestone 9: Guarded Pod Exec

Goal: add exact-Pod interactive exec without broad shell or mutation surfaces.

- [x] Add ADR 0005 and a focused Pod exec design note.
- [x] Add Pod exec command models, typed Tauri commands, and an in-memory session registry.
- [x] Add typed frontend wrappers and a Pod-only Exec detail tab.
- [x] Add explicit target and command confirmation before starting exec.
- [x] Add terminal output, stdin, resize, stop, and active-session listing.
- [x] Run frontend typecheck, frontend tests, Rust tests, and Rust check.
- [ ] Run a manual Pod exec smoke test against a readable cluster before release.

## Cross-Cutting Tracks

### Security and Safety

- [x] Verify frontend cannot invoke arbitrary shell commands.
- [x] Keep kubeconfig secrets Rust-side.
- [x] Keep the current beta inspection-first except for explicitly governed live sessions.
- [x] Add ADR 0004 for guarded cluster operations.
- [ ] Implement future cluster-changing commands as explicit, typed, permission-aware workflows.
- [ ] Add focused ADR coverage before GitOps provider API, CLI, sync, reconcile, rollback, diff, or Git-writing support.

### Helm Reconciliation

- [x] Add inspection-only Helm Reconciliation design note.
- [x] Add typed backend reconciliation command for decoded manifest intent versus live cluster state.
- [x] Replace Helm detail label-only Resources tab with backend-owned Reconciliation.
- [ ] Run a manual Helm Reconciliation smoke test against a readable cluster with Helm releases.

### Agent Skills

Tracked in detail in [agent-skills.md](agent-skills.md). Create in this order:

- [ ] `tauri-security-review`
- [ ] `kube-rs-resource-api`
- [ ] `k8s-ux-resource-browser`
- [ ] `argocd-awareness`
- [ ] Revisit `frontend-state-table-patterns` after frontend patterns settle.
- [ ] Revisit `safe-k8s-mutations` before broader guarded operation work starts.

## Later Product Areas

- guarded YAML edit/apply
- deployment-aware port-forwarding
- expanded exec scopes
- richer Helm workflows
- deeper RBAC and security inspection
- customizable workspace layout
- custom density modes
- adaptive defaults for explicit workspace types
- context-aware AI assistance
- durable local saved state
