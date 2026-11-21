# Host support

`kubecove-host` samples process usage and manages desktop connections to remote
KubeCove servers, including laptop-loopback tunnel relays.

- `usage` is available without default features, including for the headless server.
  Private `usage_webview` helpers identify platform WebView processes; shared
  attribution tests remain compiled on every platform.
- `remote-client` enables remote connections and tunnel relays for GPUI.
- `bench-support` exposes prepared process fixtures and the
  production attribution adapter through `usage::benchmark` and `usage` re-exports.
  It does not benchmark OS sampling or remote network calls.

Serialized contracts come from `kubecove-models`; borrowed runtime state and
channels come from `kubecove-kube::runtime`. Dependency versions and lint policy
are inherited from the parent Cargo workspace. Keep app setup, frontend contracts,
and benchmark harnesses in their owning crates.
