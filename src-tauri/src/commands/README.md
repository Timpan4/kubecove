# `src-tauri/src/commands/`

One file per command domain (`contexts`, `namespaces`, `resources`, `events`, `discovery`, `argo`). Shared helpers in `helpers.rs`. `mod.rs` re-exports the `#[tauri::command]` functions for `lib.rs`'s `invoke_handler!` registration.

Caps: `.rs` soft 500 lines, hard 800. If a domain file approaches the soft cap, split by sub-domain (e.g. `argo/applications.rs`, `argo/appsets.rs`). See [docs/handbook/file-size-and-split.md](../../../docs/handbook/file-size-and-split.md).

A new file in this folder must own a single command domain. Don't dump unrelated commands in an existing file because it's open already.

Note: at the time the handbook was introduced, all commands still live in a single `commands.rs` listed in `LEGACY_OVERSIZED_FILES`. New commands should be added in the per-domain layout described here.
