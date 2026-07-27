# ADR 0007: GitOps Providers Are Kubernetes-API-First And Inspection-Only By Default

## Status

Accepted. Extended for Argo CD only by [ADR 0013](0013-argocd-connected-inspection-and-operations.md); Flux remains Kubernetes-API-first and inspection-only under [ADR 0015](0015-flux-inspection-roadmap.md).

## Context

ADR 0002 made Argo CD a native product area through Kubernetes API reads of Argo CD CRDs and tracking metadata. KubeCove now needs the same product shape for Flux without turning GitOps support into CLI orchestration or a cluster-changing workflow.

Flux spans several API families: Source, Kustomize, Helm, Notification, and Image APIs. Their CRDs expose useful inspection data through Kubernetes discovery, status conditions, source references, revisions, and inventory fields. That makes them fit KubeCove's current inspection-first architecture.

## Decision

KubeCove's product surface is GitOps. Provider-specific language remains inside that surface:

- Argo CD Applications, ApplicationSets, and AppProjects.
- Flux Sources, Kustomizations, HelmReleases, Alerts, Receivers, Providers, and Image Automation resources.

GitOps provider support is Kubernetes-API-first:

- Use `kube-rs`, API discovery, and dynamic objects for provider CRDs.
- Keep existing Argo CD command names stable for compatibility.
- Add Flux detection, list, and detail commands that read Flux CRDs directly.
- Enrich resource summaries with optional GitOps ownership metadata.

At time of this decision, GitOps provider support was inspection-only by default:

- No Flux reconcile, suspend, resume, or Git-writing action is part of this decision.
- No Argo CD sync, rollback, diff, API, or CLI action is added by this decision.
- Any future cluster-changing GitOps workflow needs a focused ADR and must satisfy the guarded operation model in ADR 0004.

[ADR 0013](0013-argocd-connected-inspection-and-operations.md) now provides an explicit connected Argo CD HTTP transport for inspection and guarded operations. It is a selected transport, never a fallback. Flux remains governed by [ADR 0015](0015-flux-inspection-roadmap.md).

## Consequences

The frontend can present a single GitOps area while preserving provider-specific labels and details. Provider availability controls whether provider groups appear by default; it does not change the Kubernetes-API-first capability model.

Flux ownership can be inferred from `Kustomization.status.inventory`, `HelmRelease.status.inventory`, and Flux labels where inventory is not available.

The Kubernetes API remains the core data path. The connected Argo CD HTTP transport from ADR 0013 is explicit, not a fallback. At time of this decision, Flux CLI, Argo CD CLI, Helm CLI, and other provider integrations remained future.
