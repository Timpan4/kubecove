# Product Inspiration

## North Star

This app should use K8Studio as the strongest public feature benchmark for a local Kubernetes desktop IDE, while deliberately taking a different workflow stance:

```text
Context -> Namespace -> App / Owner -> Resource
```

K8Studio is especially useful as a reference for what a serious Kubernetes GUI can grow into: multi-cluster management, grid views, object detail panels, topology views, logs, metrics, security/RBAC views, Helm workflows, docking layouts, and AI-assisted troubleshooting.

Reference links:

- K8Studio: https://k8studio.io/
- K8Studio features: https://k8studio.io/features/
- K8Studio GitHub: https://github.com/guiqui/k8Studio

## What To Borrow As Product Ideas

- Local, agentless cluster access.
- Strong multi-cluster workflow.
- Fast searchable resource grids.
- Rich selected-object overview panels.
- Cluster and workload topology views.
- Log viewer with container and replica awareness.
- Metrics and cluster overview screens.
- RBAC, permissions, and security inspection.
- Helm release views.
- Customizable workspace layout.
- Context-aware AI assistance later.

## What To Do Differently

- Make namespace and selected context global first-class navigation state.
- Start read-only and require explicit ADRs for mutation workflows.
- Keep kubeconfig and Kubernetes access behind Rust-side Tauri commands.
- Avoid arbitrary frontend shell execution.
- Treat `kubectl`, Helm, Argo CD CLI, and terminals as optional future tools, not the default data path.
- Build Argo CD awareness as a native Kubernetes API feature before considering the Argo CD API or CLI.

## Argo CD Native Direction

Argo CD support should be native to the product, not just a label in the resource table.

The first Argo CD implementation should remain Kubernetes-API-first:

- Detect Argo-managed resources from labels and annotations.
- List Argo CD `Application`, `ApplicationSet`, and `AppProject` resources when their CRDs exist.
- Surface sync status, health status, destination namespace, source repo, revision, and project from Application status/spec.
- Group Kubernetes resources under their Argo application when tracking metadata is available.
- Add an Argo application detail view with read-only YAML, metadata, sync/health summaries, and related resources.

Later, after the core browser is solid, optional Argo CD API support can add richer app operations, history, diff, sync, rollback, and auth-aware flows. Those actions must be explicit, permission-gated, and outside the read-only MVP.

Reference links:

- Argo CD Application CRD: https://argo-cd.readthedocs.io/en/release-2.2/operator-manual/declarative-setup/#applications
- Argo CD labels and annotations: https://argo-cd.readthedocs.io/en/latest/user-guide/annotations-and-labels/
- Argo CD resource health: https://argo-cd.readthedocs.io/en/stable/operator-manual/health/

## Legal And Design Boundary

K8Studio is inspiration, not source material.

Do not copy K8Studio code, assets, branding, proprietary layouts, or marketing text. Use public feature concepts as a benchmark and design original UI around this app's namespace-first workflow.
