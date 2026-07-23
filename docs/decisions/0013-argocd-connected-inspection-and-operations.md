# ADR 0013: Argo CD Connected Inspection And Operations

## Status

Accepted.

## Decision

KubeCove keeps Kubernetes CRD inspection as explicit `kubernetes` transport. Connected Argo CD inspection uses its versioned HTTP API only after user selects an Argo server connection. There is no automatic fallback between transports.

The Rust backend owns tokens, passwords, custom CA material, TLS configuration, redirects, response limits, and API errors. Frontend receives only typed summaries. Tokens remain memory-only unless user explicitly asks native credential storage; no plaintext fallback exists. Insecure TLS is session-only and never stored.

Connected API responses provide authoritative managed-resource and `ResourceDiff` target/live/normalized/predicted state. Secret data is always redacted in Rust before serialization, regardless of frontend viewing preferences. CRD inspection may show status resources and history but never invents an exact desired state or diff.

Operations are allowlisted, typed, scoped to one Application, and preflighted. Connected refresh is an application GET with `refresh=normal|hard`; sync, rollback, terminate, and v2 resource actions use their Argo CD v3.4.5 endpoints. Retry has no endpoint: it only resubmits a recorded sync operation. Resource actions must be server-reported. Kubernetes fallback supports refresh, sync, and recorded-sync retry patches with `resourceVersion`; rollback, terminate, and resource actions are unavailable there. A short-lived single-use preflight token binds run to exact reviewed request and guards stale UI state; it is not proof against a malicious trusted frontend. No CLI, arbitrary manifests, deletes, or spec editing are allowed.

## Consequences

Manual external URLs work first. Discovered cluster Services may be presented as unavailable until a safe tunnel seam exists. This preserves Kubernetes-API-first browsing while adding opt-in connected precision.
## Connected operation contracts

Connected sync/retry payloads use Argo CD server request shapes: `syncOptions` is protobuf `StringArray` (`{ "items": [...] }`) and sync resource selectors omit `version`. Retry resolves recorded sync state during preflight, stores transformed immutable request behind one-use token, and confirms that preview before execution. Resource action discovery uses `resourceName` and server-returned `actions`; execution uses flat `ApplicationResourceActionRunRequestV2` fields.

Connected reads and operations require exact profile `clusterContext` and `workspaceId` binding. Unknown transport values are rejected.
