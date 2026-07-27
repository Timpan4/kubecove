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
- `sessions`
- `usage`

Shared command helpers belong in `helpers.rs` or a focused helper submodule. `mod.rs` re-exports command functions for `lib.rs` invoke registration.

Rules:

- One command domain per file or folder.
- Do not add unrelated commands to a file just because it is already open.
- Keep returned payloads frontend-safe.
- Keep kubeconfig contents, tokens, and certificates out of command responses.
- Live-session commands must follow ADR 0003. Other cluster-changing commands must follow ADR 0004 with explicit target scope and typed request/response models.

Caps: `.rs` soft 500 / hard 800. Split large domains by sub-domain, such as `resources/details/` or `argo/applications.rs`.

See [docs/handbook/code-organization.md](../../../docs/handbook/code-organization.md).
