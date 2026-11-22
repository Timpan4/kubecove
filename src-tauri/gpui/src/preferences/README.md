# Native preferences

`store.rs` owns version checks, native migration, credential rejection and atomic file replacement for `preferences::Document`. Callers supply paths explicitly. Default stores and parity fixtures stay in memory.

The production entry point selects the desktop settings directory and the old live-session migration source. Keep credentials and runtime handles outside this document. Local saved scopes, review notes, Argo metadata and forward presets follow ADR 0030. Remote documents remain session-only.

`tauri_migration.rs` and `store/tauri_import.rs` import Tauri 0.15.1's `tauri-migration.json` once (ADR 0036). The snapshot is read-only input.

Storage cases live in `tests/native/preferences.rs` and `preferences_migration.rs`. Tauri import cases live in `tauri_migration.rs` and `tauri_migration_lifecycle.rs`. Shared settings and restart interactions live in `e2e/parity/scenarios/preferences.json`.
