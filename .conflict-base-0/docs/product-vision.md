# Product Vision

## Identity

KubeCove is a local desktop Kubernetes workspace for operators and app developers who need fast, safe cluster work. It is not a thin `kubectl` wrapper and not a free-form mutation console. Its product stance is focused clarity under load: keep context visible, make resource state easy to scan, and move from symptoms to detail without losing the namespace or app boundary.

Current source is inspection-first with governed exceptions for port-forwarding, exact-Pod exec, selected-resource YAML apply, narrow scale/restart/delete operations, and opt-in connected Argo CD operations. Every operation keeps target scope visible and crosses a typed Rust-side boundary.

K8Studio and Aptakube are public benchmarks for capability breadth and low-friction desktop UX. KubeCove borrows product lessons, not code, branding, assets, layouts, or marketing text.

## Entry Point

The app opens into a saved workspace launcher. A workspace stores local scope only: selected contexts, namespace sets, resource filters, shortcuts, and layout preferences. Restoring a workspace refreshes live cluster state and opens a curated overview rather than replaying stale selected objects.

The overview should surface:

- cluster and namespace availability
- health and incident shortcuts
- GitOps sync and health when Argo CD or Flux metadata exists
- recent or pinned namespace, app, and resource entry points
- unavailable saved contexts, namespaces, or kinds

## Working Surface

Inside a workspace, KubeCove uses a hybrid IDE shape:

- left navigation for context, namespace, app/owner, and resource scope
- center resource table and topology/map surfaces
- right detail panel for YAML, details, events, logs, health, and incident summaries

The table answers "what exists?" The detail panel answers "what is wrong or notable?" The topology view answers "how is this connected?" Future operation surfaces must keep the same context and target visibility before acting.

## Core Jobs

1. Browse resources quickly across selected namespaces and contexts.
2. Troubleshoot incidents from health signals, warning events, restarts, logs, and YAML.
3. Understand application topology through owner references, services, ingresses, GitOps, and Helm metadata.
4. Add guarded cluster operations only after the inspection path makes the target and expected effect clear.

## Navigation Model

Namespace is first-class state, not a hidden table filter.

```text
Cluster group -> Cluster/context -> Namespace -> App/owner -> Resource
```

Supported paths:

- Namespace-first browsing for routine operational work.
- Cluster-first drilling when starting from an unfamiliar context.
- App-first jumps when GitOps, Helm, or ownership metadata identifies an application boundary.
- Multi-namespace workspaces that keep selected scope visible across tables, maps, and details.

## Multi-Cluster

Multi-cluster is part of the product model. KubeCove should support aggregate views, side-by-side compare, and named cluster groups. A cluster group is only local navigation metadata; it never changes credentials or Kubernetes API access.

Credentials remain Rust-side. The frontend receives context and resource metadata, never raw kubeconfig content, tokens, or certificates.

## Visual Direction

The baseline is balanced IDE density: compact enough for real operational work, calm enough to stay readable during an incident. Tables should be fast and scannable. Detail panels should be structured. Topology should clarify relationships rather than become decorative.

Density controls, adaptive workspace defaults, and customizable layouts are later enhancements. Defaults should remain explicit per workspace, not hidden behavior tracking.

## GitOps and Helm

GitOps and package metadata are enrichment layers, not the backbone. The Kubernetes browser must work without Argo CD, Flux, or Helm.

GitOps browsing starts Kubernetes-API-first:

- detect Argo CD CRDs and tracking metadata
- list Applications, ApplicationSets, and AppProjects
- show sync, health, destination namespace, source repo, revision, and project
- group and filter resources by Argo application
- detect Flux CRDs across Source, Kustomize, Helm, Notification, and Image APIs
- list Flux Kustomizations, Sources, HelmReleases, Alerts, Receivers, Providers, and Image Automation resources
- show Flux Ready/Reconciling/Stalled conditions, source references, revisions, suspension state, and inventory
- group and filter resources by Flux Kustomization or Flux HelmRelease when inventory or labels identify ownership

The GitOps landing view summarizes detected providers first, then links into provider-specific resource groups. Provider groups are hidden when their CRDs are not detected unless the user enables the global "Show unavailable GitOps providers" setting.

Helm support follows the same inspection-first principle: inspect release metadata and related resources without turning Helm into the core data path.

Argo CD can also use an explicit connected transport backed by its HTTP API. Connected inspection provides managed resources and target/live comparisons. Allowlisted operations cover refresh, sync, recorded-sync retry, rollback, terminate, and server-reported resource actions. Kubernetes and connected transports are explicit choices; there is no automatic fallback. Credentials, TLS, redaction, request limits, preflight tokens, and execution stay Rust-side under [ADR 0013](decisions/0013-argocd-connected-inspection-and-operations.md).

Flux remains Kubernetes-API-first and inspection-only. Argo CD CLI, Flux CLI or mutations, Helm CLI or mutations, Git-writing, and arbitrary GitOps operations remain outside the product path.

## Safety

KubeCove is inspection-first with governed operations. Port-forwarding follows [ADR 0003](decisions/0003-guarded-live-sessions.md); exact-Pod exec follows [ADR 0005](decisions/0005-guarded-pod-exec-sessions.md); selected-resource YAML apply follows [ADR 0006](decisions/0006-guarded-selected-resource-yaml-apply.md); narrow scale/restart/delete operations follow [ADR 0004](decisions/0004-guarded-cluster-operations.md); connected Argo CD operations follow [ADR 0013](decisions/0013-argocd-connected-inspection-and-operations.md).

New operations must be deliberate, permission-aware, reversible where possible, and clearly separated from browsing.

## Roadmap Shape

Near-term work should harden the current inspection workflow: workspace restore, resource tables, topology, GitOps, Helm, RBAC, metrics, events, logs, and release readiness.

Later product areas can include deployment-aware port-forwarding, broader exec scopes, richer Helm workflows, guarded Flux operations, AI-assisted troubleshooting, and durable local workspace history.
