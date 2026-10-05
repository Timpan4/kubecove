# ADR 0017: Private Argo CD Service Tunnels

## Status

Accepted 2026-08-04.

## Decision

KubeCove may connect to a discovered, selector-backed Argo CD Service through a Rust-owned, loopback-only tunnel. The frontend sends a typed Service endpoint (namespace, Service name, Service port, HTTP or HTTPS scheme, optional root path, and optional TLS server name); it never receives or displays the tunnel's local port.

Discovery reports only eligible TCP Service ports and a bounded unavailable reason when the Service cannot safely be tunneled. The backend resolves the target, starts and retains the tunnel for the connected profile, and keeps TLS configuration, custom CA material, and credentials native-side. TLS verification stays enabled unless the user explicitly enables the existing session-only override.

## Consequences

Manual external HTTPS profiles remain available. Saved profiles persist endpoint identity and scope but no credentials, TLS override, custom CA material, or tunnel address. Tunnel access is unavailable when discovery cannot prove the required Service target.

## Amendment 2026-10-05: user-confirmed tunnel target

Discovery matches Services by name (`argocd-server`, `argo-cd-argocd-server`) in any readable namespace, so a workload in any namespace can imitate Argo CD and receive a token or local login sent through a tunnel. Service names and labels are attacker-controllable and are not identity.

Trust rule:

- Discovery reports the Pod it resolved for each Service (`targetPod`) and whether the Service carries `app.kubernetes.io/part-of=argocd` (`argoLabeled`). The label is a hint only. Unlabelled Services stay selectable but are flagged in the selector and in a stronger warning.
- Before credentials are sent through a Service tunnel, including reconnects of saved profiles that use a remembered credential, the UI shows the exact namespace, Service name, and Pod with a warning and requires explicit confirmation. The user confirmation is the control.
- `connect_argo_server` takes the confirmed target (namespace, Service name, Pod). For a Service tunnel the backend starts the tunnel, resolves the target, and refuses to send any credential unless the confirmed target matches the resolved namespace, Service, and Pod. A missing confirmation or a target that changed between display and send is rejected, and the user must refresh discovery and confirm again.
- The tunnel stays pinned to the confirmed Pod. Each connection still resolves the Service, and a connection that resolves to a different Pod is refused, so a rollout or selector change requires reconnecting and confirming the new target. Pod and Service port-forward sessions (ADR 0003) keep per-connection re-resolution.
