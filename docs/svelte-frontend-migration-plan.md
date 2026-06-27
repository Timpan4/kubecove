# Svelte Frontend Cutover

## Status

Svelte is the only frontend runtime. The app starts through `src/main.ts`, which imports the Svelte entrypoint directly.

## Completed

- Svelte owns launcher, workspace shell, settings, resource browser, resource detail, YAML, events, logs, topology, GitOps, Helm, RBAC, incidents, live sessions, app updates, and usage footer surfaces.
- The runtime toggle, fallback handoff, compiler experiment config, obsolete source, and old UI dependencies have been removed.
- Settings and workspace storage remain readable by the Svelte app.
- Kubernetes access still crosses typed Tauri command wrappers only.

## Verification

Run these checks for frontend cutover changes:

```sh
bun run svelte:check
bun run typecheck
bun test
```

Manual smoke before release:

- Launch the Tauri app.
- Open a saved workspace.
- Browse resources, YAML, topology, GitOps/Helm, RBAC, incidents, live sessions, app updates, and settings.
- Confirm selected workspace, scope, resource filters, and detail state survive reload.
