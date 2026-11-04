# ADR 0029: GPUI native client and feature parity

## Status

Accepted on 2026-11-04 for an incremental native client. Full feature parity and a smaller footprint remain unproven.

## Decision

Keep Svelte for the web application and existing Tauri desktop client. Add an opt-in Rust desktop client built with [GPUI Kit](https://gpui-kit.com/) under `src-tauri/gpui/`. It has its own Cargo workspace and lockfile, like the headless server, so its GUI dependencies do not enter existing host builds.

The native client calls the existing typed Rust command implementation with desktop features disabled. Kubernetes clients, credential access, redaction, authorization, cancellation and guarded operations remain in the shared backend. The client receives the same frontend-safe models as Svelte. It does not introduce a local HTTP listener, shell access, JavaScript extensions, or a webview. GPUI Kit owns presentation and interaction, not Kubernetes policy.

The host owns a Tokio runtime and bridges results into GPUI's event loop. Scope changes cancel old work and discard stale results. Errors remain distinct from empty results. Closing the host releases its tasks. Existing Tauri-specific native file dialogs, remote tunnels and updater behavior must gain host adapters before those capabilities are declared supported.

## Initial delivery

Build context and namespace selection, resource browsing and search, selected-resource details, and YAML inspection. Exercise GPUI Kit's editor and diff viewer with local drafts. A draft diff must identify itself as local and must not imply an API-server dry-run or allow cluster execution. Later guarded actions must use the existing preview and execute commands and preserve their exact-target checks and confirmations.

Use standard GPUI Kit controls and the existing application's scope-first structure. Keep the Svelte shared UI primitives unchanged. Do not ship native automation hooks in production builds.

## Feature parity

Store frontend-independent acceptance scenarios under `e2e/parity/`. A scenario describes user actions and expected outcomes. The web and GPUI drivers interpret the same scenario; neither computes expected results from the other frontend. Drivers operate production controls rather than assigning application state.

Fast parity tests use controlled responses at the command seam. They prove UI integration, not Kubernetes access or packaged-app behavior. Native-window smoke checks and the existing isolated Kubernetes lab remain separate evidence. Every run retains reports and available screenshots under `e2e/artifacts/`. Fixtures must never fall back to a user's kubeconfig.

Track native gaps explicitly. A shared feature becomes parity-covered only after both drivers pass its scenarios. Unknown actions, missing scenarios, missing driver results and failed runs must fail the parity command. An unsupported feature cannot disappear through a silent skip. New features must add acceptance criteria for each intended client.

## Consequences

We maintain two presentation implementations while sharing backend behavior and acceptance criteria. GPUI Kit supplies table, editor, diff, focus and accessibility infrastructure, but does not port Svelte components or prove accessibility on every OS. Its pinned GPUI dependency family introduces an independent upgrade path.

Measure equivalent release builds before claiming a smaller download, lower memory use or faster startup. The native client's initial inspection scope is not a replacement for the complete web application.
