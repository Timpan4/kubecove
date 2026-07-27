# Changelog

All notable KubeCove beta releases are documented here. GitHub release notes
should mirror the matching version section.

## 0.9.1 - 2026-07-17
### Fixed

- popover stacking and resource group labels (#242)

## 0.9.0 - 2026-07-15
### Added

- Added guarded scale for Deployments and StatefulSets, rollout restart for
  Deployments, StatefulSets, and DaemonSets, and exact delete for Pods and
  ConfigMaps. Each operation uses typed targets, a dry-run preview, and explicit
  confirmation (#233).
- Added Deployment revision history backed by owned ReplicaSets, including
  side-by-side pod-template comparison (#232).
- Added pinned resources and recent namespace, application, and resource entry
  points while keeping persisted workspace state identity-only (#234).

### Improved

- Reorganized Workspace Overview around health, operations, and return-to-work
  entry points, and rebuilt Resources as a responsive GitOps-aware workbench
  (#237).
- Made the workspace shell usable at narrow desktop and mobile widths with
  drawer navigation, bounded content, and a full-width resource detail overlay
  (#231).
- Redesigned GitOps with provider-aware summary counts, a persisted Cards/List
  preference, responsive fixed-height cards, and a dense aligned list (#238).

### Fixed

- Cancelled in-flight Kubernetes queries and backend work before workspace or
  context changes, preventing stale results from the previous scope and
  coalescing rapid transitions to the newest destination (#236).
- Repaired automated releases by validating versioned macOS updater assets and
  explicitly dispatching the release workflow after tag creation (#228).

## 0.8.0 - 2026-07-11
### Added

- Share workspaces and aggregate workload logs (#209)

### Improved

- Move RBAC surface ownership
- Deepen Helm view boundary
- Move Helm surface ownership
- Tighten GitOps feature seams
- Move GitOps surface ownership
- Use public feature imports
- Move Incident surface ownership
- Move live session surface ownership
- Deepen pod exec lifecycle
- Deepen port forward lifecycle
- Route path state through navigation intents
- Keep navigation state atomic
- Centralize workspace navigation
- Unify Argo summary projections
- Deepen topology interface
- Consolidate topology transformation
- Improve topology node kind and health hierarchy
- prewarm CRDs for faster topology

### Fixed

- Synchronize Incident filter state
- Reset live sessions per workspace
- Preserve successful pod exec starts
- Fail closed when restoring port forwards
- Preserve guarded port forward starts
- address CRD prewarm review feedback

## 0.7.1 - 2026-06-30
### Fixed

- Removed the leftover Svelte launcher badge, added breathing room around the workspace view, and raised the minimum desktop window size so launcher controls are not cramped.

## 0.7.0 - 2026-06-30

### Added

- Added the Svelte app shell as the shipped frontend, including workspace,
  resource browser, resource detail, YAML, events, logs, topology, GitOps, Helm,
  RBAC, incident, live-session, settings, app update, and usage surfaces.
- Added browser dev mock mode so the Svelte frontend can run in a normal browser
  with fake Tauri responses for UI inspection, DevTools, and browser automation.
- Added release diagnostics and cancellable load reporting for local
  frontend/backend latency investigation.
- Added CodSpeed and focused frontend hot-path benchmarks for resource tables,
  sidebar trees, topology, and YAML diffs.

### Improved

- Removed the previous React runtime, fallback toggle, shadcn React primitives,
  and obsolete frontend source after the Svelte cutover.
- Improved GitOps resource grouping so controller, batch, and child resources
  appear under the expected owner chains.
- Kept owner inheritance cheaper in large resource tables by avoiding unresolved
  owner-chain scans and limiting inheritance work to displayed rows.
- Refreshed development tooling, Svelte Doctor verification, dependency update
  automation, and GitHub Actions release runners.

### Fixed

- Fixed resource view routing regressions and aligned GitOps route expectations.
- Fixed dynamic Secret redaction so secret data stays protected in dynamic
  resource detail flows.
- Fixed release tag validation so release tags must match the version on
  `origin/main`.
- Fixed Rust dependency bumps that exceeded the supported MSRV constraints.
- Hid root certificate authority noise from ownership topology.

## 0.6.7 - 2026-06-15
### Improved

- memoize ownership map flow props
- enable frontend compiler experiment
- model inspector selection as union

### Fixed

- resource table state defaults
- Address CI PR review feedback
- Svelte Doctor CI gate
- stabilize resource table columns
- make list ownership GitOps-only
- keep port-forward accept loop resilient
- restore YAML field whitespace matching
- guard app rendering and query retries
- show Argo app tracked namespaces (#147)

## 0.6.6 - 2026-06-13

### Added

- Added Kubernetes API-first Flux inspection across Source, Kustomize, Helm,
  Notification, and Image Automation CRDs, with detection, list, and detail
  commands plus provider-specific GitOps views (#142).
- Added ADR 0007 to keep GitOps providers Kubernetes-API-first and
  inspection-only by default before any sync, reconcile, rollback, CLI, or
  Git-writing workflows ship (#142).

### Improved

- Renamed the Argo CD navigation surface to GitOps while preserving existing
  Argo CD command names, saved filters, and route compatibility (#142).
- Added GitOps ownership metadata and resource filtering for Argo CD
  Applications, Flux Kustomizations, and Flux HelmReleases while preserving
  legacy Argo and Helm metadata (#142).
- Improved resource health and diagnostic detail coverage with shared backend
  health helpers, ingress status summarization, and structured frontend
  diagnostic lists (#142).

### Release

- Prepared KubeCove v0.6.6 beta release metadata across the frontend, Tauri,
  and Rust package manifests.

## 0.6.5 - 2026-06-10

### Added

- Added a global command palette opened with Command+K on macOS and Ctrl+K
  on Windows/Linux. It searches navigation targets, namespaces, and cached
  resources, then jumps directly into the matching workspace view (#137).
- Redesigned Settings around searchable sections, clearer navigation, and
  denser controls for repeat configuration work (#136).

### Improved

- Made the resource table easier to scan in large scopes with sticky headers,
  sticky Argo/kind group rows, smarter auto-hidden columns, Ready chips, and a
  tighter empty state (#140).
- Improved log reading with a line filter, compact time-only timestamps, line
  wrapping by default, and a match count while preserving full timestamp
  details on hover (#139).
- Added a YAML copy action with transient confirmation in the resource detail
  YAML tab (#139).
- Reworked status presentation across Helm, Pods, stat cards, Argo CD details,
  and the Incident Cockpit so health and sync signals are more consistent and
  less noisy (#138).

### Release

- Prepared KubeCove v0.6.5 beta release metadata across the frontend, Tauri,
  and Rust package manifests.

## 0.6.4 - 2026-06-09

### Improved

- Reused Kubernetes API clients across backend commands per kubeconfig source
  and cluster context, removing repeated kubeconfig reads, TLS setup, and cold
  connections from every resource, metrics, and session call. On-disk
  kubeconfig changes (for example cloud CLI credential refresh) still
  invalidate the cached client automatically.
- Kept the previous resource table, topology map, and metrics visible while a
  changed namespace or kind scope loads, instead of flashing a loading
  skeleton on every scope switch.

### Fixed

- Fixed ownership map node cards rendering blank (missing kind, health,
  metrics, and age badges) while a node was selected on macOS and Linux
  WebKit webviews. The selection glow now fades in via a finite transition
  instead of an infinite animation that WebKit rasterized blank.
- Fixed Svelte Doctor frontend errors across live session, resource detail,
  workspace, and resource list views.
- Fixed Rust lint verification on macOS and Linux hosts by marking the
  Windows-only WebView2 detection helpers as compiled-everywhere shared-test
  code instead of platform-dead code.

### Release

- Prepared KubeCove v0.6.4 beta release metadata across the frontend,
  Tauri, and Rust package manifests.
- Documented cross-platform rules in the agent guide: release targets,
  webview engines per platform, and CSS animation constraints for WebKit.

## 0.6.3 - 2026-06-09

### Added

- Added a persisted resource map panel preference so workspaces remember the
  table/map layout between sessions.
- Added a collapsed rail state for the resource map surface so operators can
  keep topology available without crowding the resource table.

### Fixed

- Fixed the Svelte Doctor full-scan flag used by CI verification.

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

- Fixed the Svelte Doctor CI gate and shared kubeconfig Tauri helper paths used
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

- Strengthened frontend checks around kubeconfig flows, settings, and live-session
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
  rebuilds the whole flow graph on every click.
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
- Added the core frontend workspace shell with context selection, namespace
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
