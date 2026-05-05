# Architecture Blueprint

## Product Direction

`k8s-manager` is a local desktop Kubernetes IDE with a context-first and namespace-first workflow.

Most Kubernetes tools start from resource kinds:

```text
Cluster -> Workloads -> Pods -> namespace filter -> object
```

This project should start from operational context:

```text
Cluster group -> Cluster/context -> Namespace -> App/owner -> Resource
```

The long-term differentiator is persistent global filtering across selected clusters, namespaces, resource kinds, health, owner references, Argo CD signals, and Helm release labels.

## Security Boundary

The frontend is an untrusted UI surface compared with the Rust backend.

- React must not receive raw kubeconfig contents.
- React must not run arbitrary shell commands.
- Rust Tauri commands are the only MVP path to Kubernetes data.
- Kubernetes list/get/discovery operations should use `kube-rs`.
- Destructive Kubernetes mutations are out of scope for the first milestone.

This keeps the app ready for Tauri v2 capabilities and future production-cluster guardrails.

## Backend Shape

Suggested Rust layout:

```text
src-tauri/src/
  main.rs
  commands/
    mod.rs
    kube_config.rs
    kube_resources.rs
  kube/
    mod.rs
    config.rs
    client.rs
    discovery.rs
    resources.rs
    serializers.rs
  models/
    mod.rs
    cluster.rs
    namespace.rs
    resource.rs
    error.rs
```

Responsibilities:

- `commands`: small Tauri command handlers and input validation.
- `kube/config`: load kubeconfig metadata and selected contexts without leaking secrets to the frontend.
- `kube/client`: construct context-specific `kube::Client` values.
- `kube/resources`: list/get common MVP resource types.
- `kube/discovery`: later home for dynamic resources and CRD discovery.
- `kube/serializers`: convert Kubernetes objects into frontend-safe summaries and read-only YAML.
- `models`: serde-compatible app contracts shared by commands.

## Frontend Shape

Suggested React layout:

```text
src/
  main.tsx
  app/
    router.tsx
  components/
    layout/
    ui/
  features/
    clusters/
    namespaces/
    resources/
    resource-detail/
  lib/
    tauri.ts
    types.ts
  stores/
    filters.ts
```

Responsibilities:

- `lib/tauri.ts`: typed wrappers around Tauri commands.
- `lib/types.ts`: frontend copies of the serde contracts.
- `stores/filters.ts`: selected context, namespaces, kinds, search, and drawer selection.
- `features/clusters`: context selector.
- `features/namespaces`: namespace list and multi-select.
- `features/resources`: table, filters, and query orchestration.
- `features/resource-detail`: read-only YAML, metadata, labels, annotations, owner references, and status tabs.

## Initial Commands

```text
list_kube_contexts() -> Vec<ClusterContext>
list_namespaces(context: String) -> Vec<NamespaceSummary>
list_resources(context: String, namespaces: Vec<String>, kinds: Vec<String>) -> Vec<ResourceSummary>
get_resource_yaml(context: String, api_version: String, kind: String, namespace: Option<String>, name: String) -> String
```

Errors should be returned as clean application errors and shown in the UI with enough context to diagnose kubeconfig, auth, or cluster connectivity problems.

## Initial Data Contracts

TypeScript:

```ts
export type ClusterContext = {
  name: string
  cluster?: string
  user?: string
  namespace?: string
}

export type NamespaceSummary = {
  name: string
  status?: string
  age?: string
}

export type ResourceSummary = {
  clusterContext: string
  apiVersion: string
  kind: string
  namespace?: string
  name: string
  uid?: string
  status?: string
  age?: string
  ready?: string
  restarts?: number
  owner?: string
  argoApp?: string
  helmRelease?: string
  labels?: Record<string, string>
}

export type ResourceDetails = {
  summary: ResourceSummary
  yaml: string
  metadata: Record<string, unknown>
  status?: Record<string, unknown>
}
```

Rust should expose equivalent serde structs.

## MVP Resource Strategy

Start with typed or semi-typed support for common resources:

- Pods
- Deployments
- StatefulSets
- DaemonSets
- Services
- Ingresses
- ConfigMaps
- Secrets
- PersistentVolumeClaims
- Jobs
- CronJobs
- Nodes
- Namespaces
- StorageClasses

CRDs can be added through discovery once the basic typed/common flow is stable.

## Read-Only Details

The detail panel should show:

- read-only YAML
- metadata
- labels
- annotations
- owner references
- status and conditions where available

No edit/apply/delete/scale/restart actions belong in the first milestone.

## Future Extension Points

- watch streams
- events
- logs
- YAML edit and apply
- port-forward
- pod exec
- Helm release views
- Argo CD grouping
- RBAC inspection
- metrics
- local SQLite saved state
