# ADR 0019: Reviewed RBAC risk context

## Status

Accepted on 2026-08-24.

## Context

RBAC policy heuristics intentionally flag broad permissions, including permissions required by platform controllers. Operators need to record why one exact finding is expected or anomalous without weakening or deleting the permission evidence. Durable review context is cluster-derived security metadata, so its meaning, scope, and persistence require an explicit decision.

## Decision

KubeCove keeps every RBAC risk level, reason, count, and default queue entry unchanged. A user may add one independent review record to an exact Role, ClusterRole, RoleBinding, ClusterRoleBinding, or ServiceAccount. Namespace Access remains an aggregate view and owns no review record.

A review disposition is `expected` or `anomalous`; absence means unreviewed. Only an explicit user action can set a disposition. Names, labels, owner references, GitOps metadata, and known-controller patterns may be displayed as evidence, but they never create or change a review record. Reviewing one object never changes related objects.

Each record contains only the workspace-bound cluster context, object key, compact evidence fingerprint, disposition, required note, and review timestamp. It does not persist RBAC objects, rules, subjects, credentials, or authorization results. Records use existing local workspace storage, are validated as untrusted input, and are excluded from workspace sharing exports.

The fingerprint covers the exact object's current risk, rules or subjects, affected scope, binding path, referenced permission evidence, and workload identities. A mismatch makes the record stale and removes its active disposition. KubeCove keeps the stale note visible for re-review. A stale or active record never changes the underlying risk or acts as a Kubernetes authorization verdict.

## Consequences

Expected platform-controller access becomes distinguishable from unresolved or anomalous access while the original permission remains visible. Reviews remain local to one workspace and cluster context. RBAC changes require explicit re-review instead of silently inheriting old trust.

Review notes may contain sensitive operational context. They remain local application data and are not included in shared workspace files. Exact authorization still requires a live `SubjectAccessReview`.
