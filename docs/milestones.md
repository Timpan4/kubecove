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

- [ ] Scaffold Tauri v2, React, TypeScript, Bun, and Rust project.
- [ ] Add Bun-based frontend tooling and validation scripts.
- [ ] Add Rust dependencies for Tauri commands, `kube-rs`, serde, YAML serialization, and error handling.
- [ ] Implement `list_kube_contexts`.
- [ ] Implement context-specific client construction.
- [ ] Implement `list_namespaces`.
- [ ] Implement `list_resources` for Pods, Deployments, Services, Ingresses, ConfigMaps, Secrets, PVCs, Jobs, and CronJobs.
- [ ] Implement `get_resource_yaml`.
- [ ] Build the layout with left sidebar, main resource table, and right detail panel.
- [ ] Add namespace multi-select filtering.
- [ ] Add read-only YAML/details tabs.
- [ ] Run frontend typecheck, Rust check, and app dev mode.

## Milestone 2: Usability Pass

Goal: make the browser feel useful instead of merely connected.

- [ ] Add resource search.
- [ ] Add status chips.
- [ ] Add age formatting.
- [ ] Add owner reference detection.
- [ ] Add Argo CD label and annotation detection.
- [ ] Add Helm release label detection.
- [ ] Add loading, empty, and error states.
- [ ] Persist selected context and namespaces in local UI state.

## Milestone 3: Argo CD Native Read-Only Views

Goal: make Argo CD a first-class read-only navigation and grouping layer through the Kubernetes API.

- [ ] Detect whether Argo CD CRDs exist in the selected context.
- [ ] List Argo CD Applications.
- [ ] List Argo CD ApplicationSets.
- [ ] List Argo CD AppProjects.
- [ ] Summarize Application sync status, health status, destination namespace, source repo, revision, and project.
- [ ] Group resources by Argo application when tracking metadata is available.
- [ ] Add read-only Argo application detail panel.
- [ ] Add global Argo application filter.

## Milestone 4: Discovery and Grouping

Goal: broaden Kubernetes coverage without hard-coding every resource.

- [ ] Add Kubernetes API discovery module.
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
- context-aware AI assistance
- local SQLite saved state
