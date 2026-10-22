# ADR 0025: Single Command Contract

## Status

Proposed 2026-10-22.

## Context

[ADR 0020](0020-headless-server-mode.md) made the backend command surface reachable through two transports: Tauri IPC in the desktop app and HTTP in `kubecove-server`. Each command's name, argument keys, argument types, managed state and transport class are restated in up to four hand-synchronized places:

- the desktop `tauri::generate_handler!` list in `src-tauri/src/lib.rs`;
- the server `command_table!` entries and `DESKTOP_ONLY` list in `src-tauri/server/src/dispatch/`, which repeat each camelCase key, Rust type and `State` wiring;
- the desktop remote client, which intercepts the three port-forward commands and live-session scope cleanup by string in `src-tauri/crates/host/src/remote/tunnels.rs`;
- the typed frontend wrappers in `src/lib/tauri.ts`.

Tauri derives a desktop argument key from the Rust parameter name, while the server reads the key it was given and treats a missing key as null. A misspelled optional key in the server table therefore decodes as `None` without an error. The guards are a test that parses `lib.rs` as text and a camelCase test that covers one command.

Six name-only operation commands remain registered in both transports: `preview_scale_workload`, `scale_workload`, `preview_rollout_restart`, `rollout_restart`, `preview_delete_resource` and `delete_resource`. No frontend code calls them outside their unused wrappers and browser dev mocks. `delete_resource` deletes by name with default delete parameters, without the UID and resourceVersion preconditions that [ADR 0024](0024-resource-actions-review.md) requires for exact deletion.

## Decision

### Remove the legacy operation commands

The six commands above, their TypeScript wrappers, dev mocks, request models and tests are removed. Scale, restart and delete go only through the ADR 0024 Actions family (`get_resource_action_state`, `preview_resource_action`, `execute_resource_action`), which keeps its exact-target and precondition rules. This removal happens before the contract change so the declaration does not inherit them.

### One declaration per command

`command_contract!` in `kubecove-models` declares every command once. Each entry carries its transport class and states:

- whether the command runs synchronously or asynchronously, which preserves Tauri's main-thread handling for synchronous commands;
- the command name, which is also the Rust command function in the shared `commands` API;
- its wire arguments in call order, each as the exact camelCase key the frontend sends and its Rust type;
- host-supplied parameters after the wire arguments: managed `State<T>`, the stream `Channel` read from the `channel` key, or the desktop `App` handle.

The transport classes are:

- `both`: served by Tauri IPC and server HTTP;
- `remote_intercepted`: served by both; in desktop remote mode the desktop remote client runs its own handler on the laptop instead of forwarding the call unchanged. The three port-forward commands run only on the laptop. `stop_live_sessions_outside_scope` stops the laptop's tunnelled forwards and then sends the cleanup to the server for its sessions, as the [ADR 0003](0003-guarded-live-sessions.md) amendment of 2026-10-22 requires;
- `desktop_only`: served by Tauri IPC only; the server rejects them as requiring the desktop client.

Each consumer expands the declaration with its own adapter macro:

- The desktop composition root generates one `#[tauri::command]` per entry and the `generate_handler!` list from the same entries.
- The server generates its HTTP dispatch for `both` and `remote_intercepted` entries and its desktop-only rejection from the `desktop_only` names.
- The desktop remote client generates its interception from the `remote_intercepted` entries. Each intercepted command calls a remote handler method with the same name, so a newly declared intercepted command fails to compile until the remote client handles it.

Tauri IPC and server HTTP are two real adapters for this seam. The remote interception is an override inside the desktop remote client, not a third transport.

### Shared argument decoding

Every adapter decodes arguments with the same `CommandArgs` type in `kubecove-models`, reading the key written in the declaration. A payload must be a JSON object. A missing key reads as null, so an optional argument becomes `None`; a missing or invalid required argument fails with a validation `AppError` that names the key. Both adapters serialize results with `serde_json` and return the same `AppError` shape.

Tauri no longer derives argument keys from Rust parameter names. Generated IPC commands receive the raw request and decode it with `CommandArgs`, so the declared key is the only spelling in either transport. A key mismatch between transports cannot be expressed. Command functions keep their names, take wire arguments before host-supplied parameters, and return `Result<T, AppError>`. The frontend keeps its command names and camelCase argument keys.

Malformed IPC arguments now produce the validation `AppError` that the server already returned, instead of Tauri's string error. Correct frontend calls are unaffected.

### ADR 0020 explicit table and test

ADR 0020 requires the server to dispatch through an explicit command table and a test that classifies every desktop command. The declaration is now that explicit table, and each entry's transport class is the classification. Neither transport can serve a command that is not declared. Both adapters expand the same entries and decode with the same `CommandArgs` calls on the same key tokens, so a key mismatch between them cannot compile into existence. Tests then check what construction does not:

- every declared key uses the lowerCamel form the frontend sends, and command names and keys are unique;
- every declared command is run through the HTTP adapter: a non-object payload fails validation, a payload with an invalid value for every declared key fails on the first declared key, and desktop-only commands are rejected before any argument is read;
- unknown command names are rejected over HTTP.

These replace the test that parsed `lib.rs` as text and the single-command camelCase test. A runtime test of the IPC adapter would need Tauri's `test` feature as a dev-dependency, which the GUI-free CI job cannot build without WebKit; the IPC side therefore relies on the shared expansion and decoding above.

### Scope classification

This decision does not classify commands by the operator, user or client-session scope proposed in [ADR 0023](0023-multi-user-sessions-and-persistence.md), which is still Proposed. Each entry already names its host-supplied state, and `kubecove-models` exposes the entries as the `DECLARED_COMMANDS` inventory. An entry can gain a scope field later without another list.

## Consequences

Adding a command takes one declaration entry, the Rust command function and its typed frontend wrapper. The desktop handler list, server dispatch, desktop-only rejection and remote interception follow from the declaration.

The frontend wrappers in `src/lib/tauri.ts` stay hand-written. This decision does not generate TypeScript, so a frontend key that differs from the declaration still needs typecheck, frontend tests or E2E coverage to catch.

Domain crates no longer carry `#[tauri::command]` attributes. They still use the `runtime` `State` and `Channel` types, so desktop and server builds keep their existing feature split.
