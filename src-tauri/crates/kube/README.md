# Kubernetes infrastructure

Shared kubeconfig source/settings handling, three Kubernetes client paths, session-local bearer/caches/diagnostics, cancellation guards, live cache and common resource metadata/projection helpers. `runtime` preserves Tauri state/channel types on desktop and scope-inheriting Tokio tasks on the server. E2E-only source validation stays feature-gated here so all command crates use the same isolated source.

Depends only on models within this workspace. It must not depend on inspection, GitOps, live-session registries or a host. Application-wide cancellation belongs in the composition root. The Flux kind catalog and dynamic projections live here to prevent resource ownership and Helm from depending back on command domains.
