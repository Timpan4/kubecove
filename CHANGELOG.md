# Changelog

All notable KubeCove beta releases are documented here. GitHub release notes
should mirror the matching version section.

## 0.6.4 - 2026-06-09
### Fixed

- stop WebKit blanking topology cards on selection (#133)
- React Doctor frontend errors (#122)

## 0.6.3 - 2026-06-09

### Added

- Added a persisted resource map panel preference so workspaces remember the
  table/map layout between sessions.
- Added a collapsed rail state for the resource map surface so operators can
  keep topology available without crowding the resource table.

### Fixed

- Fixed the React Doctor full-scan flag used by CI verification.

### Release

- Prepared KubeCove v0.6.3 beta release metadata across the frontend,
  Tauri, and Rust package manifests.

## 0.6.2 - 2026-06-09

### Improved

- Hardened the resource workspace for large and busy scopes by splitting the
  app shell, keeping resource views responsive, and tightening topology/table
  state handling.
- Enforced strict Rust Clippy linting across the backend verification baseline.

### Fixed

- Fixed the React Doctor CI gate and shared kubeconfig Tauri helper paths used
  by resource views.

### Release

- Prepared KubeCove v0.6.2 beta release metadata across the frontend,
  Tauri, and Rust package manifests.

## 0.6.1 - 2026-06-08

### Added

- Added a guarded YAML apply force-conflicts option with global settings and a
  selected-resource override for field ownership conflict retries.

### Fixed

- Fixed release tag identity handling in the tag workflow.
- Fixed macOS release asset checks so release validation matches the published
  artifact names.

### Release

- Prepared KubeCove v0.6.1 beta release metadata across the frontend,
  Tauri, and Rust package manifests.

## 0.6.0 - 2026-06-08

### Added

- Added kubeconfig source tracking so commands and live-session cleanup can
  respond to workspace and kubeconfig scope changes.
- Added release automation for preparing release PRs, tagging releases, yanking
  releases, and generating release notes from the changelog.

### Improved

- Strengthened React checks around kubeconfig flows, settings, and live-session
  helpers.
- Added release tests and documentation for the beta release workflow.

### Fixed

- Fixed live-session workspace cleanup so port-forward and exec sessions outside
  the active workspace or kubeconfig source scope are stopped consistently.
- Fixed live-session kubeconfig source build issues and aligned related helper
  typings and port-forward action expectations.
- Fixed and then reverted the release PR token change after workflow behavior
  proved unsafe for the release path.

### Release

- Prepared KubeCove v0.6.0 beta release metadata across the frontend,
  Tauri, and Rust package manifests.

## 0.5.0 - 2026-06-08

### Added

- Added guarded selected-resource YAML apply with apply-friendly YAML, server-side
  dry-run, diff review, Secret apply protection, and explicit final
  confirmation.
- Added Helm reconciliation so release details compare decoded Helm intent with
  live cluster resources.
- Added ADR 0006 for the selected-resource YAML apply guardrail contract.

### Improved

- Fixed the YAML edit flow so apply preparation and refresh behavior stay
  aligned with the selected resource.
- Improved ownership topology selection with animated flow paths that respect
  reduced-motion preferences.
- Tightened WebView process-tree detection for local browser/process tooling.

### Release

- Prepared KubeCove v0.5.0 beta release metadata across the frontend,
  Tauri, and Rust package manifests.

## 0.4.3 - 2026-06-06

### Improved

- Tightened the resource-browser shortcut path so overview Resources opens the
  saved workspace scope directly.
- Made sidebar chevron clicks select the same node they expand, reducing split
  behavior between row labels and disclosure controls.
- Added release smoke guidance for Windows foregrounding and guarded Pod exec.

### Release

- Prepared KubeCove v0.4.3 beta release metadata across the frontend,
  Tauri, and Rust package manifests.

## 0.4.2 - 2026-06-04

### Added

- Added guarded Pod exec sessions with exact-Pod targeting, typed Tauri
  commands, confirmation guardrails, terminal UI, and lifecycle cleanup.
- Added agent-facing domain, issue-tracker, and triage-label docs for more
  consistent project automation.

### Fixed

- Kept saved port-forward presets available across namespace filter changes
  when the workspace context still matches.

### Release

- Prepared KubeCove v0.4.2 beta release metadata across the frontend,
  Tauri, and Rust package manifests.

## 0.4.1 - 2026-06-01

### Improved

- Kept Service port-forward sessions connected by re-resolving ready backing Pods
  when local connections reconnect or sessions restart.
- Fit ownership topology paths into view more reliably after selecting related
  resources.
- Added clearer topology health badge colors.
- Improved GitHub Actions dependency caching for CI and release builds.

### Release

- Prepared KubeCove v0.4.1 beta release metadata across the frontend,
  Tauri, and Rust package manifests.

## 0.4.0 - 2026-06-01

### Added

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
