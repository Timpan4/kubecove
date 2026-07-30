# ADR 0016: Deepen GitOps And Finite-Read Modules

## Status

Accepted on 2026-07-30.

## Context

Recent Argo CD, resource, and workspace changes exposed shallow modules: callers coordinate profile policy, repeated Application reads, operation phases, finite-read cleanup, and provider-specific ownership presentation. Source-text tests verify those call sites rather than behavior through stable interfaces. Deleting the current helpers removes little complexity because their important rules remain distributed.

## Decision

KubeCove will deepen five distinct modules rather than combine GitOps, transport, operations, and lifecycle state behind one broad interface:

1. **Workspace-scoped Argo connection policy** owns exact-workspace eligibility, saved-order automatic choice, and persisted Automatic, Kubernetes, or Connected profile preference. The policy remains in-process in the frontend; existing persistence and typed Tauri adapters remain internal seams. Profile identifiers stay stable.
2. **Argo Application inspection** owns one coherent Application result, eager resource comparisons, visible whole-result Connected-to-Kubernetes fallback, redaction, provenance, and one lazy resource-action read. Connected and Kubernetes transports are two real internal adapters. Detailed behavior is governed by [ADR 0013](0013-argocd-connected-inspection-and-operations.md).
3. **Guarded Argo operation session** owns review, transport selection before confirmation, five-minute session persistence, restart revalidation, single-use execution, and typed outcomes in Rust. Native credential storage and an in-memory test adapter form a real internal seam. Execution never changes reviewed transport.
4. **Finite-read lifecycle** owns TanStack Query metadata, request identity, next-task exact-query observer checks, frontend cancellation, and typed backend cancellation. It uses TanStack Query directly rather than adding a replacement query interface. Finite reads, confirmed writes, and long-lived sessions remain separate under [ADR 0009](0009-workspace-request-cancellation.md).
5. **GitOps ownership evidence** owns normalized evidence and presentation for both Argo CD and Flux. Argo and Flux are two real provider adapters. Resource Browser, Resource Detail, topology, and Incident Workbench consume this module without learning provider-specific inventory, labels, confidence, filtering, or presentation rules.

Each module exposes one small interface shared by callers and tests. Internal seams stay private. No extension seam is added for one implementation; two adapters are required before a seam becomes public architecture.

Existing shallow source-text tests and helper tests will be replaced, not layered, by behavior tests through these interfaces. Trust-boundary validation, Secret redaction, exact workspace scope, guarded-operation review, ADR 0009 cancellation ordering, and partial GitOps ownership coverage remain observable requirements.

## Consequences

Implementation lands as a staged stack: decision records, finite-read lifecycle, connection policy, Application inspection, GitOps ownership evidence, then operation sessions. Each layer must replace its old path before obsolete commands, helpers, and tests are removed.

The frontend retains locality for workspace preference, TanStack observers, and presentation. Rust retains locality for credentials, transport reads, response bounds, redaction, operation review, and execution. Callers gain leverage from coherent results and stop reproducing transport, cancellation, and provider rules.
