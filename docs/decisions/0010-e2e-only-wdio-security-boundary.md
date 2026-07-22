# ADR 0010: E2E-Only WDIO Security Boundary

## Status

Accepted.

## Context

Native end-to-end tests must exercise the shipped Tauri command boundary and real `kube-rs` behavior. WebDriver support expands the desktop application's permissions and exposes a global Tauri API, so enabling it in normal development or release builds would weaken the production boundary. Real tests also create Kubernetes objects and must never discover a user's persisted kubeconfig by accident.

## Decision

Native end-to-end tests use a dedicated Cargo `e2e` feature and a separate Tauri configuration. That feature alone compiles and registers `tauri-plugin-wdio` and its embedded WebDriver provider. The E2E configuration alone enables the WDIO capability, global Tauri API, incognito WebView storage, and the development release channel. Normal development and release configuration contains none of those hooks.

Startup is symmetric and fail closed:

- an `e2e` feature build requires `KUBECOVE_E2E=1`;
- a build without the feature refuses `KUBECOVE_E2E=1`;
- native E2E requires absolute `KUBECOVE_KUBECONFIG` and `KUBECOVE_DATA_DIR` paths plus the exact `KUBECOVE_E2E_CLUSTER` name;
- the generated kubeconfig must contain only the expected admin and restricted contexts, both targeting a loopback API server for that run cluster;
- E2E mode ignores persisted kubeconfig sources, user `KUBECONFIG`, and default kubeconfig discovery.

The fast Chrome suite is different by design. It opens the normal Vite development frontend and uses the existing typed browser mocks. It does not inject Tauri globals or load Rust.

The lifecycle runner owns each real run. It records an exact generated cluster name before cleanup becomes eligible, gathers redacted diagnostics, then deletes only that cluster and its own temporary directory. Raw kubeconfig contents, tokens, keys, certificates, and environment dumps are never artifacts. `--keep` is local-only and rejected in CI.

`dev:kind` does not enable the E2E feature. It uses normal application code with a generated kubeconfig and temporary application/WebView profile. Users may add another source manually during that disposable session. Its workspace-specific cluster persists until `dev:kind:down`, which deletes only the exact hashed cluster name.

## Consequences

Native tests can exercise real Tauri commands without shipping a production automation surface. A misconfigured E2E process stops before it can fall back to a user's cluster. The dedicated flavor adds build time and must be checked separately from the default flavor. Any future desktop automation permission or test-only command requires an update to this ADR and a production-build absence check.
