# ADR 0032: Desktop close guard for unsaved YAML drafts

## Status

Accepted 2026-11-09 for the Tauri desktop client. Extends [ADR 0029](0029-gpui-native-client-and-feature-parity.md), which requires the Svelte and GPUI clients to keep their behavior aligned.

## Context

The GPUI inspector refuses to close the resource detail while its YAML draft has unsaved changes. The Svelte client already refuses the navigation actions that would replace or close the open draft. It calls `canLeaveYamlDraft()` before those actions and shows `UNSAVED_YAML_DRAFT_NOTICE`.

Closing the window was not guarded in either web runtime:

- In browser mode, commit 0a2a34f adds a `beforeunload` guard. The browser asks before the page unloads while the draft has changes.
- In the Tauri desktop build, the window emits a close request. `onCloseRequested` in `@tauri-apps/api` runs the handler, then calls `destroy()` unless the handler called `preventDefault()`. The handler must call `preventDefault()` before it returns.
- The `@tauri-apps/api` documentation for `destroy()` says it requires `core:window:allow-destroy`, which is not included in `core:window:default`. The current `src-tauri/capabilities/default.json` grants `core:default` and no window destroy permission, so a handler that allows the close cannot destroy the window through that API.

## Decision

1. Grant `core:window:allow-destroy` in `src-tauri/capabilities/default.json`, scoped to the `main` window as that capability already scopes its permissions.
2. Register `onCloseRequested` once in the Svelte app, and only when `isTauriRuntime()` is true. The handler calls `canLeaveYamlDraft()`. When that returns false, the handler calls `preventDefault()`, and the existing in-page notice explains why the window stayed open.
3. Keep the browser `beforeunload` guard for browser mode only. The same runtime check gates it, so a Tauri build does not run both guards.

The desktop guard uses the existing draft registry and notice. It adds no new draft state and no new Tauri command.

## Scope

- The guard covers the close request for the main desktop window. It is the only window the capability names.
- It covers only the open YAML draft, the same draft that `canLeaveYamlDraft()` checks today.
- It does not cover app exit or process restart. Tauri does not route those through the window close request, so a draft can still be lost through them. Covering them needs a separate decision.
- Exec sessions, port-forward sessions and other open work keep their existing behavior. This ADR does not change them.

## Security impact

`core:window:allow-destroy` enables the window `destroy` command for the `main` window with no scope. The capability does not add data access or a new IPC route for Kubernetes or the filesystem. The frontend code that gains the permission is the app's own webview code, which already holds the cluster data it renders. The permission lets that code destroy its own window only. It does not let the frontend close other windows, because the capability is limited to `main`.

The change keeps the existing boundary: the frontend still has no kubeconfig or shell access, and Rust still owns Kubernetes access.

## Alternatives considered

- **Rust `on_window_event` handler with a typed command.** Rust would receive `WindowEvent::CloseRequested` and ask the frontend, through a new typed command, whether the draft allows closing. This keeps the decision in Rust, but the draft state lives in the Svelte app, so the command would only relay a yes or no. It adds a command contract and a round trip for one check. Rejected for now. Revisit it if app exit must be guarded too, because Rust can see exit paths that the frontend cannot.
- **Browser mode only.** This leaves the desktop app closable with an unsaved draft. The GPUI client refuses that close, so the two clients would differ. Rejected.

## Verification

Before merge, check the desktop build on Linux, macOS and Windows:

- Close with a clean draft. The window closes.
- Close with an unsaved draft. The window stays open and the notice appears in the page.
- Discard the draft, then close again. The window closes.

These checks are needed because the webview engines differ: WebKitGTK on Linux, WKWebView on macOS, and WebView2 on Windows. The shared parity driver cannot send a Tauri close request to the browser mock, so these checks are not covered by parity scenarios.

The browser `beforeunload` guard from commit 0a2a34f has no parity scenario either. Check it by hand in browser mode: edit a draft, then try to close or reload the tab.
