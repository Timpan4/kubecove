# ADR 0003: Guarded Live Kubernetes Sessions

## Status

Accepted.

## Context

KubeCove is inspection-first by default and now supports governed live-session work where the target and lifecycle are explicit. Port-forward and exec are different from list, get, watch, logs, and metrics because they open live operational sessions into a cluster. They still must respect the existing local desktop boundary: Kubernetes credentials stay Rust-side, the frontend uses typed Tauri commands, and the app does not shell out to `kubectl`.

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

## Amendment 2026-10-09: tunnelled forwards in desktop remote mode

When the desktop app uses a remote `kubecove-server` ([ADR 0020](0020-headless-server-mode.md)), a port-forward may listen on the laptop instead of the server:

- The desktop app binds the listener to `127.0.0.1` on the laptop under the port rules above. The server opens no TCP listener for a tunnelled forward.
- Each accepted local connection opens a numbered stream on that client session's authenticated WebSocket. The server checks that the forward belongs to that session, resolves the target as it does for local forwards, Service re-resolution included, and relays bytes through the Kubernetes port-forward API.
- Relay buffers are bounded by the existing session buffer sizes. A full buffer pauses reads from the sending side instead of dropping bytes. Frames for unknown, closed, or other-session ids close that stream.
- A tunnelled forward stops on explicit stop, session failure, scope changes as above, desktop app exit, explicit session closure, or expiry of that session's server reconnect grace window. Under ADR 0020's concurrent-session amendment of 2026-10-11, another sign-in opens an independent session and does not transfer or stop existing forwards.

Browser clients cannot open local listeners, so their forwards keep listening on the server's `127.0.0.1`, and the UI labels them as remote.

## Amendment 2026-10-22: one scope rule for every forward

Status: Proposed.

The amendment of 2026-10-09 says tunnelled forwards stop on scope changes, but it does not say which kubeconfig source a tunnelled forward belongs to or which side stops it. The same rule now applies to every port-forward, whether its listener runs in the backend that resolves it or on the laptop:

- Each forward records its cluster context and the kubeconfig source key of the backend that validated and resolved its target. A tunnelled forward records the server's source key, which the server returns when it registers the forward.
- When live-session retention is not enabled and the workspace scope changes, a forward keeps running only if its cluster context is one of the workspace's contexts and its source key equals the workspace's source key. A forward without a recorded source key is outside every scope.
- In desktop remote mode, the desktop app applies the rule to its tunnelled forwards. It closes their laptop listeners, unregisters them on the server, and then sends the same cleanup to the server for its sessions. The cleanup result counts tunnelled forwards with the other stopped forwards.
- Pod exec sessions ([ADR 0005](0005-guarded-pod-exec-sessions.md)) keep their own lifecycle and apply the same scope rule.
- The private Argo CD Service tunnel ([ADR 0017](0017-private-argocd-service-tunnels.md)) is not a port-forward session. Its Argo CD connection owns it, and live-session cleanup does not stop it.
