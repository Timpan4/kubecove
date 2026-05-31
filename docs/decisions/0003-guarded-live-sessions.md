# ADR 0003: Guarded Live Kubernetes Sessions

## Status

Accepted.

## Context

KubeCove is inspection-first by default and now supports governed live-session work where the target and lifecycle are explicit. Port-forward and exec are different from list, get, watch, logs, and metrics because they open live operational sessions into a cluster. They still must respect the existing local desktop boundary: Kubernetes credentials stay Rust-side, React uses typed Tauri commands, and the app does not shell out to `kubectl`.

## Decision

Live session support starts with pod port-forwarding only.

Port-forward v1 rules:

- Targets are exact Pods in a namespace. Services, Deployments, and replacement-pod resolution are future work.
- Sessions start only from explicit user action and stop only from explicit user action, app exit, or session failure.
- Local listeners bind only to `127.0.0.1`.
- Local port `0` is used only internally for automatic port selection. User-provided local ports must be unprivileged.
- Sessions are in-memory only and are not restored across app restarts.
- Kubernetes RBAC remains authoritative for `pods/portforward`; forbidden errors are surfaced as command errors or session errors.
- The frontend never receives kubeconfig contents, tokens, certificates, or broad filesystem access.
- The frontend does not receive shell execution capability.

Exec remains out of scope for this ADR's implementation. Any exec UI must add a separate design pass for command constraints, stdin handling, terminal behavior, output retention, and audit UX. Broader cluster-changing operations follow [ADR 0004](0004-guarded-cluster-operations.md).

## Consequences

Port-forwarding becomes the first live operational workflow in KubeCove without changing the app into a generic shell wrapper. The implementation adds a WebSocket-capable `kube-rs` path and an in-memory session registry, but keeps cluster access behind typed Rust commands and keeps active tunnels visible and stoppable from the UI.
