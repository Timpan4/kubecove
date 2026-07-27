# ADR 0017: Private Argo CD Service Tunnels

## Status

Accepted 2026-08-04.

## Decision

KubeCove may connect to a discovered, selector-backed Argo CD Service through a Rust-owned, loopback-only tunnel. The frontend sends a typed Service endpoint (namespace, Service name, Service port, HTTP or HTTPS scheme, optional root path, and optional TLS server name); it never receives or displays the tunnel's local port.

Discovery reports only eligible TCP Service ports and a bounded unavailable reason when the Service cannot safely be tunneled. The backend resolves the target, starts and retains the tunnel for the connected profile, and keeps TLS configuration, custom CA material, and credentials native-side. TLS verification stays enabled unless the user explicitly enables the existing session-only override.

## Consequences

Manual external HTTPS profiles remain available. Saved profiles persist endpoint identity and scope but no credentials, TLS override, custom CA material, or tunnel address. Tunnel access is unavailable when discovery cannot prove the required Service target.
