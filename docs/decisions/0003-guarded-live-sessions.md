# ADR 0003: Guarded Live Kubernetes Sessions

## Status

Accepted.

## Context

KubeCove is inspection-first by default and now supports governed live-session work where the target and lifecycle are explicit. Port-forward and exec are different from list, get, watch, logs, and metrics because they open live operational sessions into a cluster. They still must respect the existing local desktop boundary: Kubernetes credentials stay Rust-side, React uses typed Tauri commands, and the app does not shell out to `kubectl`.

## Decision

Live session support starts with guarded port-forwarding only.

Port-forward v1 rules:

- Targets are exact Pods or selector-backed Services in a namespace. Service targets resolve to one ready backing Pod at session start and re-resolve on reconnect or explicit restart.
- Selectorless Services, ExternalName Services, and Services without ready matching Pods are rejected with readable errors.
- Deployments and replacement-pod resolution are future work.
- Sessions start only from explicit user action and stop from explicit user action, app exit, session failure, or workspace/kubeconfig source scope changes when live-session retention is not enabled.
- Local listeners bind only to `127.0.0.1`.
- Local port `0` is used only internally for automatic port selection. User-provided local ports must be unprivileged.
- Sessions are in-memory only and are not restored across app restarts. Workspace switches cut sessions outside the new workspace context and kubeconfig source scope by default; a user setting may keep them running.
- Service sessions re-resolve to a ready backing Pod when a local connection reconnects or a session is explicitly restarted. They do not run as durable background controllers and are still not restored across app restarts.
- Kubernetes RBAC remains authoritative for `pods/portforward`; forbidden errors are surfaced as command errors or session errors.
- The frontend never receives kubeconfig contents, tokens, certificates, or broad filesystem access.
- The frontend does not receive shell execution capability.

Exec remains out of scope for this ADR's implementation. Any exec UI must add a separate design pass for command constraints, stdin handling, terminal behavior, output retention, and audit UX. Broader cluster-changing operations follow [ADR 0004](0004-guarded-cluster-operations.md).

## Consequences

Port-forwarding becomes the first live operational workflow in KubeCove without changing the app into a generic shell wrapper. The implementation adds a WebSocket-capable `kube-rs` path, Service-to-Pod resolution, and an in-memory session registry, but keeps cluster access behind typed Rust commands and keeps active tunnels visible and stoppable from the UI.
