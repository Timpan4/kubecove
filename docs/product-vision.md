# Product Vision

## Identity

KubeCove is a local desktop Kubernetes workspace for operators and app developers who need fast, safe cluster exploration. It is not a thin `kubectl` wrapper and not a mutation-heavy IDE. Its product stance is focused clarity under load: keep context visible, make resource state easy to scan, and move from symptoms to detail without losing the namespace or app boundary.

K8Studio and Aptakube are public benchmarks for capability breadth and low-friction desktop UX. KubeCove borrows product lessons, not code, branding, assets, layouts, or marketing text.

## Entry Point

The app opens into a saved workspace launcher. A workspace stores local scope only: selected contexts, namespace sets, resource filters, shortcuts, and layout preferences. Restoring a workspace refreshes live cluster state and opens a curated overview rather than replaying stale selected objects.

The overview should surface:

- cluster and namespace availability
- health and incident shortcuts
- Argo CD sync and health when Argo metadata exists
- recent or pinned namespace, app, and resource entry points
- unavailable saved contexts, namespaces, or kinds

## Working Surface

Inside a workspace, KubeCove uses a hybrid IDE shape:

- left navigation for context, namespace, app/owner, and resource scope
- center resource table and topology/map surfaces
- right detail panel for YAML, details, events, logs, health, and incident summaries

The table answers "what exists?" The detail panel answers "what is wrong or notable?" The topology view answers "how is this connected?"

## Core Jobs

1. Browse resources quickly across selected namespaces and contexts.
2. Troubleshoot incidents from health signals, warning events, restarts, logs, and YAML.
3. Understand application topology through owner references, services, ingresses, Argo CD, and Helm metadata.

## Navigation Model

Namespace is first-class state, not a hidden table filter.

```text
Cluster group -> Cluster/context -> Namespace -> App/owner -> Resource
```

Supported paths:

- Namespace-first browsing for routine operational work.
- Cluster-first drilling when starting from an unfamiliar context.
- App-first jumps when Argo CD, Helm, or ownership metadata identifies an application boundary.
- Multi-namespace workspaces that keep selected scope visible across tables, maps, and details.

## Multi-Cluster

Multi-cluster is part of the product model. KubeCove should support aggregate views, side-by-side compare, and named cluster groups. A cluster group is only local navigation metadata; it never changes credentials or Kubernetes API access.

Credentials remain Rust-side. The frontend receives context and resource metadata, never raw kubeconfig content, tokens, or certificates.

## Visual Direction

The baseline is balanced IDE density: compact enough for real operational work, calm enough to stay readable during an incident. Tables should be fast and scannable. Detail panels should be structured. Topology should clarify relationships rather than become decorative.

Density controls, adaptive workspace defaults, and customizable layouts are later enhancements. Defaults should remain explicit per workspace, not hidden behavior tracking.

## GitOps and Helm

GitOps and package metadata are enrichment layers, not the backbone. The Kubernetes browser must work without Argo CD or Helm.

Argo CD support starts Kubernetes-API-first:

- detect Argo CD CRDs and tracking metadata
- list Applications, ApplicationSets, and AppProjects
- show sync, health, destination namespace, source repo, revision, and project
- group and filter resources by Argo application

Helm support should follow the same read-only principle: inspect release metadata and related resources without turning Helm into the core data path.

Argo CD API, Argo CD CLI, Helm CLI, sync, rollback, diff, and mutation workflows require separate ADRs and guardrails.

## Safety

The current product is read-only. No create, update, delete, scale, restart, sync, rollback, exec, or port-forward workflow should be exposed as a normal path without a new ADR.

Future mutations must be deliberate, permission-aware, reversible where possible, and clearly separated from default browsing.

## Roadmap Shape

Near-term work should harden the current read-only incident workflow: workspace restore, resource tables, topology, Argo CD, Helm, RBAC, metrics, events, logs, and release readiness.

Later product areas can include guarded YAML apply, port-forward, pod exec, richer Helm workflows, deeper RBAC/security inspection, AI-assisted troubleshooting, and durable local workspace history.
