# Product Inspiration

## Benchmarks

KubeCove uses public products as capability benchmarks while keeping its own workflow and visual language.

- K8Studio: broad Kubernetes IDE surface area, including multi-cluster management, grid views, object details, topology, logs, metrics, RBAC/security, Helm, docking layouts, and AI-assisted troubleshooting.
- Aptakube: clean local desktop UX, fast resource tables, approachable multi-cluster browsing, and a polished operator workflow.

Reference links:

- K8Studio: https://k8studio.io/
- K8Studio features: https://k8studio.io/features/
- K8Studio GitHub: https://github.com/guiqui/k8Studio
- Aptakube: https://aptakube.com/

## Borrow as Product Ideas

- Local, agentless cluster access.
- Strong multi-cluster workflow.
- Fast searchable resource grids.
- Rich selected-object detail panels.
- Cluster and workload topology views.
- Log viewer with container awareness.
- Metrics and cluster overview screens.
- RBAC, permissions, and security inspection.
- Helm release views.
- Customizable workspace layout.
- Context-aware AI assistance later.

## Do Differently

- Make context and namespace global navigation state.
- Restore saved workspaces into live curated overviews, not stale exact UI state.
- Start read-only and require ADRs for mutation workflows.
- Keep kubeconfig and Kubernetes API access behind Rust-side Tauri commands.
- Avoid arbitrary frontend shell execution.
- Treat `kubectl`, Helm CLI, Argo CD CLI, and terminals as optional future sidecars or fallbacks.
- Build Argo CD awareness through Kubernetes API resources before considering the Argo CD API or CLI.

## Argo CD Direction

Argo CD should be native product context, not just a badge.

The first class path remains Kubernetes-API-first:

- detect Argo-managed resources from labels and annotations
- list `Application`, `ApplicationSet`, and `AppProject` resources when CRDs exist
- surface sync status, health status, destination namespace, source repo, revision, and project
- group Kubernetes resources under their Argo application when tracking metadata exists
- show read-only YAML, metadata, sync/health summaries, and related resources

Later Argo CD API support may add richer history, diff, sync, rollback, and auth-aware flows. Those features require explicit ADRs and permission-gated UX.

Reference links:

- Argo CD Application CRD: https://argo-cd.readthedocs.io/en/release-2.2/operator-manual/declarative-setup/#applications
- Argo CD labels and annotations: https://argo-cd.readthedocs.io/en/latest/user-guide/annotations-and-labels/
- Argo CD resource health: https://argo-cd.readthedocs.io/en/stable/operator-manual/health/

## Boundary

K8Studio and Aptakube are inspiration only. Do not copy their code, assets, branding, proprietary layouts, or marketing text. Use public feature concepts as benchmarks and design KubeCove around its context-first, namespace-first workflow.
