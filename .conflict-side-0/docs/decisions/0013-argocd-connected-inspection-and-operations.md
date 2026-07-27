# ADR 0013: Argo CD Connected Inspection And Operations

## Status

Accepted. Revised on 2026-08-04; private Service tunnels follow [ADR 0017](0017-private-argocd-service-tunnels.md).

## Decision

KubeCove keeps Kubernetes CRD inspection as `kubernetes` transport and Connected Argo CD inspection as `connected` transport through its versioned HTTP interface. Each workspace persists one inspection preference: automatic selection, Kubernetes transport, or one Connected Argo CD profile. Automatic selection uses the first healthy profile in saved order whose `clusterContext` and `workspaceId` exactly match. Existing profile identifiers remain stable. An explicit Kubernetes preference skips Connected inspection; an explicit Connected preference never substitutes another profile.

A Connected inspection attempt is whole-result. It reads the Application and managed resources, eagerly derives every available resource comparison, and returns only Connected data when successful. Resource actions load lazily for one selected resource. If any required Connected inspection read fails, KubeCove discards partial Connected data and visibly returns one complete Kubernetes inspection when possible. The result preserves a sanitized Connected failure and identifies Kubernetes provenance. It never mixes transports, hides fallback, or invents exact desired state from Kubernetes status. If both transports fail, the error preserves both safe failure classifications.

The Rust backend owns tokens, passwords, custom CA material, TLS configuration, redirects, response limits, Secret redaction, transport fallback, and errors. Frontend receives only typed, bounded results. Tokens remain memory-only unless user explicitly asks native credential storage; no plaintext fallback exists. Insecure TLS is session-only and never stored. Connected responses remain authoritative when successful, and Secret data is redacted in Rust before serialization regardless of frontend viewing preferences.

Operations remain allowlisted, typed, scoped to one Application, and reviewed before execution. Review may try Connected transport first and, only when that attempt cannot produce a review, use the existing Kubernetes adapter for supported refresh, sync, or recorded-sync retry actions. Rollback, terminate, and resource actions remain unavailable through Kubernetes transport. The resulting review visibly names its transport and exact target. Execution is bound to that reviewed transport and never falls back or switches after confirmation.

Rust owns each operation review as a five-minute, single-use session stored only in native credential storage. The session contains the canonical reviewed request, exact workspace and Application binding, transport, issue and expiry times, and no credentials. Execution consumes the session once. After an app restart, Rust revalidates exact scope, credential or client availability, Application identity, authorization, and action availability before execution. A failed or ambiguous execution cannot replay the consumed session. No plaintext, browser-storage, or memory-only persistence fallback is allowed when secure storage is unavailable.

Connected refresh is an Application GET with `refresh=normal|hard`; sync, rollback, terminate, and v2 resource actions use their Argo CD v3.4.5 endpoints. Retry has no endpoint: it only resubmits a recorded sync operation. Resource actions must be server-reported. Kubernetes review supports refresh, sync, and recorded-sync retry patches with `resourceVersion`; rollback, terminate, and resource actions are unavailable there. No CLI, arbitrary manifests, deletes, or spec editing are allowed.

## Consequences

Read-only Application inspection remains useful when a configured Argo CD server is unavailable, but fallback loss of precision stays visible. One coherent inspection replaces caller-managed Application, managed-resource, and per-resource comparison reads. Automatic profile choice is deterministic and workspace-scoped; explicit choices remain authoritative.

Operation review can recover through the existing Kubernetes adapter before confirmation, but execution cannot change what user reviewed. Persisted sessions can survive a short restart without persisting credentials or weakening live revalidation. Native secure storage failure makes operation review unavailable rather than reducing storage safety.

Manual external HTTPS URLs work first. Discovered eligible cluster Services can use a private tunnel under ADR 0017; unavailable discovery remains explicit.

## Connected operation contracts

Connected sync/retry payloads use Argo CD server request shapes: `syncOptions` is protobuf `StringArray` (`{ "items": [...] }`) and sync resource selectors omit `version`. Retry resolves recorded sync state during review, stores transformed immutable request in the single-use session, and confirms that review before execution. Resource action discovery uses `resourceName` and server-returned `actions`; execution uses flat `ApplicationResourceActionRunRequestV2` fields.

Connected reads and operations require exact profile `clusterContext` and `workspaceId` binding. Unknown transport values are rejected.
