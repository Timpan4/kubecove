# Milestones

## Milestone 0: Repository Foundation

Goal: create the project container for implementation work.

- [x] Initialize git workspace.
- [x] Capture architecture constraints.
- [x] Capture milestone plan.
- [x] Capture agent guidance.
- [x] Create GitHub repository and push the foundation commit.

## Milestone 1: Read-Only Kubernetes Browser

Goal: working local desktop app that can browse common Kubernetes resources through Rust-side Tauri commands.

- [x] Scaffold Tauri v2, React, TypeScript, Bun, and Rust project.
- [x] Add Bun-based frontend tooling and validation scripts.
- [x] Add Rust dependencies for Tauri commands, `kube-rs`, serde, YAML serialization, and error handling.
- [x] Implement `list_kube_contexts`.
- [x] Implement context-specific client construction.
- [x] Implement `list_namespaces`.
- [x] Implement `list_resources` for Pods, Deployments, Services, Ingresses, ConfigMaps, Secrets, PVCs, Jobs, and CronJobs.
- [x] Implement `get_resource_yaml`.
- [x] Build the layout with left sidebar, main resource table, and right detail panel.
- [x] Add namespace multi-select filtering.
- [x] Add read-only YAML/details tabs.
- [x] Run frontend typecheck, Rust check, and app dev mode.

## Milestone 2: Usability Pass

Goal: make the browser feel useful instead of merely connected.

- [x] Add resource search.
- [x] Add status chips.
- [x] Add age formatting.
- [x] Add owner reference detection.
- [x] Add Argo CD label and annotation detection.
- [x] Add Helm release label detection.
- [x] Add loading, empty, and error states.
- [x] Persist selected context and namespaces in local UI state.

## Milestone 3: Argo CD Native Read-Only Views

Goal: make Argo CD a first-class read-only navigation and grouping layer through the Kubernetes API.

- [x] Detect whether Argo CD CRDs exist in the selected context.
- [x] List Argo CD Applications.
- [x] List Argo CD ApplicationSets.
- [x] List Argo CD AppProjects.
- [x] Summarize Application sync status, health status, destination namespace, source repo, revision, and project.
- [x] Group resources by Argo application when tracking metadata is available.
- [x] Add read-only Argo application detail panel.
- [x] Add global Argo application filter.

## Milestone 4: Discovery and Grouping

Goal: broaden Kubernetes coverage without hard-coding every resource.

- [x] Add Kubernetes API discovery module.
- [ ] Add CRD listing if practical.
- [ ] Add dynamic-object listing for discovered resources.
- [ ] Add resource kind filter backed by discovery.
- [ ] Add app/owner grouping views.

## K8Studio-Inspired Later Milestones

- topology and relationship maps
- logs
- events
- watches
- YAML edit/apply with explicit guardrails
- port-forward
- pod exec
- Helm views
- RBAC inspection
- permissions and security inspection
- metrics
- customizable workspace layout
- saved workspace launcher and curated restore
- custom density modes
- adaptive defaults for explicit workspace types
- context-aware AI assistance
- local SQLite saved state
