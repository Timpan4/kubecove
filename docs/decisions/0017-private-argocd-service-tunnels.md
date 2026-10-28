# ADR 0017: Private Argo CD Service Tunnels

## Status

Accepted 2026-08-04. Amended 2026-10-05 and 2026-10-28.

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

## Amendment 2026-10-28: workspace setup, Service-level reconnect, and certificate pinning

Setup moves from global Settings to the workspace GitOps view and Application details, because profiles were already bound to one workspace, cluster context, and kubeconfig source. The user chose to trust a confirmed namespace and Service across rollouts, provided HTTPS stays bound to the certificate they confirmed. This replaces the Pod pinning in the 2026-10-05 amendment.

Discovery:

- A Service is a candidate when its name is `argocd-server`, `argo-cd-argocd-server`, or ends in `-argocd-server`; when it carries `app.kubernetes.io/name=argocd-server`; or when it carries `app.kubernetes.io/part-of=argocd` and `app.kubernetes.io/component=server` and its name does not end in `-metrics`. Names and labels remain hints, not identity.
- Discovery reads `argocd-cmd-params-cm` in the Service namespace. `server.insecure=true` selects HTTP; otherwise HTTPS. `server.rootpath` sets the root path. An unreadable ConfigMap falls back to HTTPS at `/`. The port named for the protocol, or its default port, is listed first.
- When listing Services cluster-wide is forbidden, discovery lists Services in the namespaces that hold Argo CD Applications. If none of those are readable, discovery returns the original forbidden error.

Trust rule:

- `preview_argo_tunnel_target` starts a tunnel without credentials and returns the Pod it reaches. For HTTPS it also returns the SHA-256 fingerprint of the server certificate. The probe verifier rejects every certificate, so the handshake ends before any HTTP request.
- A first connection, and any connection with newly entered credentials, requires confirming the namespace, Service, Pod, and, for HTTPS, the certificate fingerprint. The backend refuses credentials unless the confirmed namespace, Service, and Pod match the tunnel it started.
- HTTPS through a tunnel accepts exactly the confirmed certificate and no other, regardless of CA, hostname, or expiry. Insecure TLS and custom CA material are rejected for tunnel endpoints. They remain session-only options for external HTTPS profiles.
- With **Remember credential**, the fingerprint is stored with the token in native credential storage, never in settings. A certificate change rotates the credential generation.
- A reconnect may confirm only the namespace and Service, with no Pod. The backend accepts that only when no credential is submitted and a remembered credential exists. The remembered credential was saved after a full confirmation. HTTPS reconnects use the stored fingerprint, and a profile without one must be confirmed again. If the server presents a different certificate, the handshake fails before credentials are sent and the user confirms again.
- When a workspace opens, KubeCove reconnects its remembered profiles once without prompting. A failure leaves the profile disconnected and is shown when the user reconnects from the connection dialog.
- Each tunnel connection resolves the confirmed Service and forwards to the Pod it reaches at that time, so the tunnel follows rollouts. HTTP tunnels have no certificate binding: any Pod behind the confirmed Service can receive the credential.
