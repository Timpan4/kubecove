# Project TODOs

## Next Session: Scaffold the App

- [ ] Create Tauri v2 + React + TypeScript app in this repo using Bun.
- [ ] Choose and install the component/UI baseline.
- [ ] Add TanStack Router, Query, and Table.
- [ ] Choose Zustand or Jotai for local UI state.
- [ ] Add Tailwind CSS.
- [ ] Add Bun scripts for frontend typecheck, Rust check, and dev mode.
- [ ] Confirm the generated app starts locally.

## Backend MVP

- [ ] Define Rust serde models for `ClusterContext`, `NamespaceSummary`, `ResourceSummary`, and `ResourceDetails`.
- [ ] Add app error type that serializes cleanly to the frontend.
- [ ] Implement kubeconfig context listing without exposing kubeconfig contents.
- [ ] Build context-specific Kubernetes clients.
- [ ] List namespaces.
- [ ] List MVP namespaced resources.
- [ ] List basic cluster-scoped resources.
- [ ] Convert resource objects into summaries.
- [ ] Serialize full objects to read-only YAML.

## Frontend MVP

- [ ] Add typed Tauri command wrappers.
- [ ] Add context selector.
- [ ] Add namespace selector with multi-select behavior.
- [ ] Add resource category navigation.
- [ ] Add TanStack resource table.
- [ ] Add detail drawer/panel.
- [ ] Add YAML, metadata, and status tabs.
- [ ] Keep UI state local and explicit.

## Security and Safety

- [ ] Verify frontend cannot invoke arbitrary shell commands.
- [ ] Keep kubeconfig secrets Rust-side.
- [ ] Keep first milestone read-only.
- [ ] Make future mutation commands explicit and permission-gated.

## Nice Soon

- [ ] Search/filter bar.
- [ ] Status chips.
- [ ] Age formatting.
- [ ] Argo and Helm label detection.
- [ ] Owner reference display.
- [ ] Basic app/owner grouping experiment.
