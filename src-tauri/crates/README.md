# Private backend crates

The desktop package at `src-tauri/` is the Cargo workspace root and host composition layer. Internal crates are unpublished and versioned independently of app releases. Dependencies are pinned in `[workspace.dependencies]`; members opt into the existing backend lint policy with `[lints] workspace = true`.

| Crate | Responsibility | Internal dependencies |
| --- | --- | --- |
| `kubecove-models` | Serialized contracts, health assessment, endpoint validation and typed errors | None |
| `kubecove-kube` | Kubernetes clients, source settings, session scope, cancellation primitives, diagnostics, common metadata/projections and live cache | models |
| `kubecove-inspection` | Resource reads, discovery, topology, metrics, RBAC, incidents and guarded Kubernetes operations | models, kube |
| `kubecove-live` | Watches, logs, exec, the Port Forward Lifecycle with its shared accept-and-relay loop, and session cleanup | models, kube |
| `kubecove-gitops` | Argo CD, Flux and Helm | models, kube, live |
| `kubecove-host` | Process usage, native workspace files and desktop remote client, including desktop-tunnelled forwards | models, kube, live (desktop only) |

`kubecove` aggregates the existing public `commands`, `models` and `runtime` APIs for the desktop and server. Workspace cancellation stays there because it coordinates client generations, live-cache loads and Argo connections. Domain crates must not depend on this composition root. The `commands` namespaces preserve existing domain paths; they are not separate implementations.

## Build boundaries

Each member defaults to no desktop features. The shell forwards `desktop`, `bench-support` and E2E isolation explicitly. Models contain existing Kubernetes error conversions and URL validation, so they are not a serde-only package. `bench-support` deliberately exposes deterministic projection/fixture adapters without adding commands to the frontend.

Keep session task-local state, client caches and runtime spawning in the same crate: every host must use exactly the same scope and cancellation-marker type. Keep finite reads, confirmed operations and live client generations distinct. Splitting code must not change IPC payloads, guarded targets, redaction, credentials or cleanup order.

The server is excluded from the desktop workspace. It retains its own lockfile and `server/target` directory, and consumes the shared facade with default features disabled. This prevents a desktop workspace build from unifying Tauri into the server. Do not add it to `--workspace --all-features` checks.

## Verification and rebuild measurements

```sh
cargo test --locked --workspace --manifest-path src-tauri/Cargo.toml
cargo clippy --locked --workspace --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
cargo test --locked --workspace --no-default-features --manifest-path src-tauri/Cargo.toml
cargo test --locked --no-default-features --manifest-path src-tauri/server/Cargo.toml
cargo tree --locked --manifest-path src-tauri/server/Cargo.toml --edges normal,build
```

Use the workspace selectors when testing: dependency-crate unit tests do not run merely because the shell depends on them. Use the standalone server command for GUI-free verification.

Build-turnaround evidence and repeatable workload details belong in [Rust build measurements](../../docs/rust-build-measurements.md). These are compiler/linker measurements, not runtime CodSpeed benchmarks. Dependency-heavy cold builds and final linking remain costs after the split.
