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

Shared command helpers belong in `helpers.rs` or a focused helper submodule. `mod.rs` re-exports the command functions that `command_contract!` in `kubecove-models` declares. Tauri IPC (`src/ipc.rs`) and the server HTTP dispatch are generated from that declaration; see [ADR 0025](../../../docs/decisions/0025-single-command-contract.md).

Rules:

- One command domain per file or folder.
- Do not add unrelated commands to a file just because it is already open.
- Declare each new command once in `command_contract!`, with its camelCase wire keys and transport class.
- Command functions take wire arguments before managed state, channels or the app handle, and return `Result<T, AppError>`.
- Keep returned payloads frontend-safe.
- Keep kubeconfig contents, tokens, and certificates out of command responses.
- Live-session commands must follow ADR 0003. Other cluster-changing commands must follow ADR 0004 with explicit target scope and typed request/response models.

Caps: `.rs` soft 500 / hard 800. Split large domains by sub-domain, such as `resources/details/` or `argo/applications.rs`.

See [docs/handbook/code-organization.md](../../../docs/handbook/code-organization.md).
