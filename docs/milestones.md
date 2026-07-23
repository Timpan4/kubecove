# Milestones

This is the goal-level progress tracker. Keep it factual: check a box only when the app behavior exists and the relevant verification has run or is clearly recorded elsewhere.

Latest published release is `0.9.1`. Current source includes unreleased work completed after that tag; version metadata changes only through the release PR flow.

## Milestone 0: Repository Foundation

Goal: create the project container for implementation work.

- [x] Initialize git workspace.
- [x] Capture architecture constraints.
- [x] Capture milestone plan.
- [x] Capture agent guidance.
- [x] Create GitHub repository and push the foundation commit.

## Milestone 1: Kubernetes Inspection Browser

Goal: browse common Kubernetes resources through Rust-side Tauri commands.

- [x] Scaffold Tauri v2, frontend TypeScript, Bun, and Rust.
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
- [x] Add release diagnostics mode for local frontend/backend latency reports.
- [ ] Run a manual Tauri smoke test against a readable cluster before the next beta release.

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

## Milestone 10: Svelte Frontend Migration

Goal: finish the Svelte cutover and keep the frontend path simple.

- [x] Add Svelte SPA entrypoint.
- [x] Add runtime badge.
- [x] Preserve shared settings and workspace storage schemas through cutover.
- [x] Port launcher, workspace shell, settings, resource browser, resource detail, YAML, events, logs, topology, GitOps, Helm, RBAC, incidents, live sessions, app updates, and usage footer surfaces to Svelte.
- [x] Run automated Svelte migration baseline on 2026-06-22: `bun run svelte:check`, `bun run typecheck`, and `bun test`.
- [x] Make Svelte the default for new installs.
- [x] Remove the previous frontend runtime, fallback toggle, compiler config, and obsolete source.
- [ ] Record browser-backed app-wide launch, resource list, YAML detail, and 4k topology measurements.
- [ ] Run manual Svelte Tauri smoke against a readable cluster.

## Cross-Cutting Tracks

### Security and Safety

- [x] Verify frontend cannot invoke arbitrary shell commands.
- [x] Keep kubeconfig secrets Rust-side.
- [x] Keep the product inspection-first except for explicitly governed live sessions and operations.
- [x] Add ADR 0004 for guarded cluster operations.
- [x] Implement guarded scale (Deployment/StatefulSet), rollout restart (Deployment/StatefulSet/DaemonSet), and exact delete (Pod/ConfigMap) as explicit, typed, permission-aware workflows.
- [x] Add ADR 0013 for connected Argo CD inspection and allowlisted guarded operations.
- [ ] Add focused ADR coverage before Flux mutation, provider CLI integration, Git-writing, or Helm mutation support.

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

### Deployment Revision History

- [x] Add read-only Deployment ReplicaSet revision history and pod-template comparison.

## Milestone 11: Deterministic E2E Lab

Goal: make browser behavior, native Tauri boundaries, and the supported Kubernetes range reproducible without touching user clusters.

- [x] Add the WDIO fast Chrome path over existing typed DEV mocks.
- [x] Add the E2E-only native Tauri flavor and fail-closed kubeconfig boundary.
- [x] Add the Kind lifecycle runner, deterministic fixtures, exact cleanup, and redacted diagnostics.
- [x] Add workspace-specific `dev:kind` and exact `dev:kind:down` commands.
- [x] Add ADR 0010 for the E2E-only WDIO security boundary.
- [x] Add ADR 0011 for rolling latest-three-minor Kubernetes support, initially 1.34–1.36.
- [x] Add fast PR coverage, probationary 1.35 coverage, nightly/manual matrices, native desktop smoke, and the release matrix.
- [x] Replace static GitOps placeholders with the production-shaped Cilium, Argo CD, metrics, storage, ingress, tenant, Helm, and incident lab from ADR 0012.
- [ ] Promote the Ubuntu 1.35 real suite into the required aggregate check after 10 consecutive green default-branch or nightly runs.

## Milestone 12: Connected Argo CD Inspection and Operations

Goal: add opt-in Argo CD API precision without weakening Kubernetes-first browsing or backend security boundaries.

- [x] Add ADR 0013 for explicit Kubernetes and connected transports with no automatic fallback.
- [x] Add Argo server discovery, explicit connection profiles, memory-only credentials by default, optional native credential storage, custom CA support, and session-only insecure TLS.
- [x] Add connected application inspection, managed resources, and target/live/normalized/predicted resource comparison.
- [x] Add preflighted refresh, sync, recorded-sync retry, rollback, terminate, and server-reported resource actions.
- [x] Keep Kubernetes fallback limited to refresh, sync, and recorded-sync retry patches.
- [x] Add ADR 0014 and transient per-key Secret reveal while redacting connected Argo Secret payloads in Rust.
- [x] Add ADR 0015 to keep Flux Kubernetes-API-first and inspection-only.
- [ ] Run manual connected-Argo smoke against a readable Argo CD 3.4 server before the next release.

## Later Product Areas

- deployment-aware port-forwarding
- expanded exec scopes
- richer Helm workflows
- guarded Flux operations
- customizable workspace layout
- custom density modes
- adaptive defaults for explicit workspace types
- context-aware AI assistance
- optional teaching-mode Kubernetes error explanations
- durable local saved state
