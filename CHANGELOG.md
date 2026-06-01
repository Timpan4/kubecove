# Changelog

All notable KubeCove beta releases are documented here. GitHub release notes
should mirror the matching version section.

## 0.4.0 - 2026-06-01

### Added

- Added the workspace Port Forwards manager with saved Service presets and
  guarded restore prompts for local-only live sessions.
- Added reproducible frontend and resource-scope performance benchmarks for
  sidebar expansion, topology selection, indexed search, and cache retention.

### Improved

- Cut retained sidebar tree objects by lazily building namespace children only
  when a namespace is expanded.
- Reused topology layout and selection indexes so selecting resources no longer
  rebuilds the whole React Flow graph on every click.
- Indexed table search text once per resource refresh instead of rebuilding
  searchable fields for every search pass.
- Bounded backend ready-cache retention and kept namespace-scoped reads and
  watches namespace-scoped for limited-RBAC users.

### Fixed

- Fixed standalone topology bucket dimming when selected paths include owned
  resources of the same kind.
- Fixed live-store cache trimming so loading entries do not force eviction below
  the intended ready-entry budget.
- Removed stale namespace-coalescing benchmark claims after preserving
  namespace-scoped Kubernetes access for RBAC safety.

### Release

- Prepared KubeCove v0.4.0 beta release metadata across the frontend,
  Tauri, and Rust package manifests.

## 0.3.0 - 2026-05-26

### Added

- Added read-only Helm release views with release summaries, detail panels,
  and related Kubernetes resource context.
- Added read-only RBAC security inspection for namespace access, roles,
  bindings, service accounts, and risk summaries.
- Added optional read-only resource metrics when cluster metrics APIs are
  available.
- Added network flow topology and incident triage timelines for faster
  troubleshooting.
- Added workspace cluster groups for organizing local multi-cluster scopes.

### Improved

- Made resource filters selectable so active context, namespace, and kind scope
  are easier to adjust while browsing.
- Improved release smoke-test documentation and updater release-channel gating.

### Release

- Prepared KubeCove v0.3.0 beta release metadata across the frontend,
  Tauri, and Rust package manifests.

## 0.2.1 - 2026-05-21

### Release

- Prepared KubeCove v0.2.1 beta release metadata across the frontend,
  Tauri, and Rust package manifests.

## 0.2.0 - 2026-05-20

### Added

- Added an ownership topology map for browsing workload and service
  relationships next to the resource table.
- Added incident-focused resource detail summaries for unhealthy resources,
  warning events, restart signals, and quick workspace shortcuts.
- Added release-readiness improvements, including a usage footer and stronger
  release version validation.

### Improved

- Shared Kubernetes resource fetches across live views so tables, topology, and
  workspace summaries can reuse fresher backend state.
- Improved topology map routing, node presentation, resize handling, and
  panning boundaries.
- Kept table, topology map, and detail selection better synchronized during
  incident inspection.
- Refreshed the release toolchain and published current macOS, Windows, and
  Linux beta installer assets.

### Fixed

- Fixed stale live-store entries, namespace-scoped live fetches, and final load
  handling for watch-backed resource updates.
- Fixed ownership map viewport observer behavior after layout changes.
- Stabilized the current frontend and Rust verification baseline.

### Release

- Added AGPL-3.0-or-later license metadata.
- Published KubeCove v0.2.0 as the latest beta release.

## 0.1.0 - 2026-05-18

### Added

- Published the first KubeCove beta with a local read-only Kubernetes browser.
- Added Rust-side Tauri commands for kube context discovery, namespace listing,
  resource listing, resource YAML, and resource details.
- Added the core React workspace shell with context selection, namespace
  filtering, resource tables, and read-only YAML/details panels.
- Added Argo CD and Helm metadata detection, owner reference summaries, resource
  search, status chips, age formatting, and persisted dashboard state.
- Added Kubernetes API discovery-backed resource coverage and dynamic resource
  listing.
- Added saved workspace launcher flows for creating, opening, editing, deleting,
  and restoring workspace scopes.

### Improved

- Refined the KubeCove brand, logo, UI surfaces, settings, resource timestamps,
  and detail timelines.
- Split backend and frontend resource modules to keep the codebase easier to
  review and extend.
- Added repository handbook guidance, file-size guardrails, pre-commit checks,
  and release automation scripts.

### Fixed

- Fixed restart signal handling, stale Argo detection, unsupported-kind
  feedback, workspace namespace error handling, and sidebar regressions.
- Fixed release tag validation so beta releases are created from matching
  `app-v*` tags and version metadata.

### Release

- Added unsigned beta installer publishing for GitHub Releases.
- Published KubeCove v0.1.0 as the initial public beta release.
