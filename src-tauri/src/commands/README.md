# `src-tauri/src/commands/`

Tauri command handlers live here, split by domain.

Current domains include:

- `contexts`
- `namespaces`
- `resources`
- `events`
- `discovery`
- `streams`
- `metrics`
- `argo`
- `helm`
- `rbac`
- `usage`

Shared command helpers belong in `helpers.rs` or a focused helper submodule. `mod.rs` re-exports command functions for `lib.rs` invoke registration.

Rules:

- One command domain per file or folder.
- Do not add unrelated commands to a file just because it is already open.
- Keep returned payloads frontend-safe.
- Keep kubeconfig contents, tokens, and certificates out of command responses.

Caps: `.rs` soft 500 / hard 800. Split large domains by sub-domain, such as `resources/details/` or `argo/applications.rs`.

See [docs/handbook/code-organization.md](../../../docs/handbook/code-organization.md).
