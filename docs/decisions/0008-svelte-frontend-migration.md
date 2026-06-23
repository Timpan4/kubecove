# ADR 0008: Parallel Svelte Frontend Migration

## Status

Accepted. Svelte may become the default UI once the cutover gates in this ADR pass; React remains available as a fallback for one release after that default switch.

## Context

KubeCove is a Tauri desktop app whose current frontend is React. The product is stateful: workspaces, selected cluster scope, resource inspection, YAML, topology, GitOps, Helm, RBAC, incidents, settings, live sessions, and diagnostics all share local UI state and typed Tauri command wrappers.

A Svelte topology spike showed promising manual performance and bundle-size signals. The spike is not enough to justify replacing the whole frontend by itself. A full migration must prove app-wide value, preserve the current security boundary, and keep a React fallback until parity is complete.

Astro remains out of scope because KubeCove is a stateful desktop app shell, not a content site.

## Decision

KubeCove may pursue a React-to-Svelte frontend migration as a parallel app, not an in-place rewrite and not a big-bang cutover.

The migration rules are:

- Keep React available until Svelte passes the cutover gates, then ship Svelte as the default with a React fallback for one release.
- Build Svelte behind a user-visible Settings toggle during beta.
- The toggle writes local frontend settings state and reloads the webview cleanly.
- Show a runtime identity badge in the top bar using small React/Svelte logo assets so testers can see which UI is active.
- Show a toast when switching UI mode before reload.
- Clicking the runtime badge opens Settings to the UI mode control.
- Preserve existing settings and workspace storage schemas so React and Svelte can read and write the same user data.
- Keep typed Tauri wrappers, TypeScript contract types, query keys, and pure resource/topology/workspace helpers shared.
- Do not change Tauri command names, payloads, Kubernetes access paths, or cluster-changing behavior as part of this migration.
- Do not embed React islands inside Svelte. Unported Svelte surfaces should offer a clear action to switch to React mode and reload.

The planned Svelte stack is:

- `svelte`
- `@sveltejs/vite-plugin-svelte`
- `svelte-check`
- `@tanstack/svelte-query`
- `@tanstack/svelte-table`
- Bits UI for headless primitives
- `lucide-svelte`
- `@xyflow/svelte` for topology, if it reaches UX parity

## Cutover Gates

Svelte can become the default UI only when all of these are true:

- Daily core parity passes for launcher/workspaces, cluster and scope navigation, resource list, resource detail, YAML, topology, GitOps and Helm read paths, settings, and live-session list/status.
- App-wide launch, resource list, YAML, and topology measurements are better than or neutral to React.
- At least one major app-wide bottleneck improves by 25% or more before the project continues beyond early migration slices.
- The runtime badge and Settings toggle make fallback state obvious.
- The React fallback still works for every unported tail surface.

React can be removed only after tail parity lands for command palette, incidents, RBAC, app updates, usage footer, and current edge states.

If `@xyflow/svelte` cannot match the required topology UX, Svelte cannot become the default UI. A custom topology renderer would need a separate decision.

## Consequences

The migration will temporarily carry two frontend runtimes and duplicate UI shell code. That cost is intentional: it keeps the shipped app usable while Svelte earns default status with measured value.

The migration must be delivered through small vertical PRs. Each slice should keep both React and Svelte builds working until React is removed.

The frontend remains an untrusted UI surface. Kubernetes credentials stay behind Rust-side typed Tauri commands, and future cluster-changing work remains governed by the existing guarded-operation ADRs.
