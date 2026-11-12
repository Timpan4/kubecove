# `src-tauri/src/commands/`

This folder holds `mod.rs` and benchmark support. Command handlers live in the domain crates under `src-tauri/crates/<crate>/src/commands/` (`kube`, `inspection`, `live`, `gitops`, `host`); see the [crates guide](../../crates/README.md). `mod.rs` re-exports the command functions that `command_contract!` in `kubecove-models` declares. Tauri IPC (`src/ipc.rs`) and the server HTTP dispatch are generated from that declaration; see [ADR 0025](../../../docs/decisions/0025-single-command-contract.md).

Rules for command code in those crates:

- One command domain per file or folder.
- Do not add unrelated commands to a file just because it is already open.
- Declare each new command once in `command_contract!`, with its camelCase wire keys and transport class.
- Command functions take wire arguments before managed state, channels or the app handle, and return `Result<T, AppError>`.
- Keep returned payloads frontend-safe.
- Keep kubeconfig contents, tokens, and certificates out of command responses.
- Live-session commands must follow ADR 0003. Other cluster-changing commands must follow ADR 0004 with explicit target scope and typed request/response models.

Caps: `.rs` soft 500 / hard 800. Split large domains by sub-domain, such as `resources/details/` or `argo/applications.rs`.

See [docs/handbook/code-organization.md](../../../docs/handbook/code-organization.md).
