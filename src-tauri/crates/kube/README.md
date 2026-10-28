# Kubernetes infrastructure

Shared kubeconfig source settings, cluster access by intent (ADR 0026), session-local bearer/caches/diagnostics, cancellation guards, live cache and common resource metadata/projection helpers. `runtime` preserves Tauri state/channel types on desktop and scope-inheriting Tokio tasks on the server. E2E-only source validation stays feature-gated here so all command crates use the same isolated source.

Commands reach Kubernetes only through `ClusterAccess`: resolve it once per request from the selector, then ask it for a read, preview, confirmed-operation, live-session or stream client. It owns source resolution, the ADR 0009 client paths and the fail-closed rule. The `test-support` feature exposes `FixedClusterAccess`, which other crates enable from their dev-dependencies to run commands against `tower_test` mock clients.

Depends only on models within this workspace. It must not depend on inspection, GitOps, live-session registries or a host. Application-wide cancellation belongs in the composition root. The Flux kind catalog and dynamic projections live here to prevent resource ownership and Helm from depending back on command domains.
