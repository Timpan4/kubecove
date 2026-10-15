# Host support

`kubecove-host` owns process-usage sampling and attribution, desktop connections to
remote KubeCove servers (including laptop-loopback tunnel relays), and native
workspace JSON import/export dialogs. It does not own Kubernetes access or app
startup and command registration.

- `usage` is available without default features, including for the headless server.
  Private `usage_webview` helpers identify platform WebView processes; shared
  attribution tests remain compiled on every platform.
- `remote` and `workspace_files` require `desktop`. That feature enables the native
  dependencies and `kubecove-kube/desktop` so runtime state and channels use the
  same Tauri types as the desktop shell.
- `bench-support` enables `desktop` and exposes prepared process fixtures and the
  production attribution adapter through `usage::benchmark` and `usage` re-exports.
  It does not benchmark OS sampling or remote network calls.

Serialized contracts come from `kubecove-models`; borrowed runtime state and
channels come from `kubecove-kube::runtime`. Dependency versions and lint policy
are inherited from the parent Cargo workspace. Keep app setup, frontend contracts,
and benchmark harnesses in their owning crates.
