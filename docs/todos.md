# Project TODOs

## Next Session: Scaffold the App

- [ ] Enable local hooks with `git config core.hooksPath .githooks`.
- [ ] Create Tauri v2 + React + TypeScript app in this repo using Bun.
- [ ] Choose and install the component/UI baseline.
- [x] Add TanStack Router, Query, and Table.
- [x] Choose Zustand or Jotai for local UI state.
- [ ] Add Tailwind CSS.
- [x] Add Bun scripts for frontend typecheck, Rust check, and dev mode.
- [x] Confirm the generated app starts locally.

## Backend MVP

- [x] Define Rust serde models for `ClusterContext`, `NamespaceSummary`, `ResourceSummary`, and `ResourceDetails`.
- [x] Add app error type that serializes cleanly to the frontend.
- [x] Implement kubeconfig context listing without exposing kubeconfig contents.
- [x] Build context-specific Kubernetes clients.
- [x] List namespaces.
- [x] List MVP namespaced resources.
- [ ] List basic cluster-scoped resources.
- [x] Convert resource objects into summaries.
- [x] Serialize full objects to read-only YAML.
- [x] Keep resource summary fields ready for Argo app and Helm release grouping.

## Frontend MVP

- [x] Add typed Tauri command wrappers.
- [x] Add context selector.
- [x] Add namespace selector with multi-select behavior.
- [x] Add resource category navigation.
- [x] Add TanStack resource table.
- [x] Add detail drawer/panel.
- [x] Add YAML, metadata, and status tabs.
- [x] Keep UI state local and explicit.

## Security and Safety

- [ ] Verify frontend cannot invoke arbitrary shell commands.
- [x] Keep kubeconfig secrets Rust-side.
- [x] Keep first milestone read-only.
- [ ] Make future mutation commands explicit and permission-gated.

## Agent Skills

- [ ] Create and validate `tauri-security-review`.
- [ ] Create and validate `kube-rs-resource-api`.
- [ ] Create and validate `k8s-ux-resource-browser`.
- [ ] Create and validate `argocd-awareness`.
- [ ] Revisit `frontend-state-table-patterns` after frontend patterns exist.
- [ ] Revisit `safe-k8s-mutations` before mutation work starts.

## Nice Soon

- [x] Search/filter bar.
- [x] Status chips.
- [x] Age formatting.
- [x] Argo and Helm label detection.
- [x] Owner reference display.
- [ ] Basic app/owner grouping experiment.

## Argo CD Native

- [ ] Detect Argo CD CRDs.
- [ ] List Applications, ApplicationSets, and AppProjects through the Kubernetes API.
- [ ] Add Argo application summaries.
- [ ] Add Argo application detail view.
- [ ] Group resources by Argo tracking metadata.
- [ ] Add ADR before Argo CD API, CLI, sync, rollback, or diff support.

## K8Studio-Inspired Backlog

- [ ] Topology and relationship maps.
- [ ] Advanced logs.
- [ ] Cluster overview and metrics.
- [ ] RBAC and user permissions views.
- [ ] Security inspection views.
- [ ] Helm release views.
- [ ] Customizable workspace layout.
- [ ] Context-aware AI assistance.
