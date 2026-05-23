# Calm Mission Control UI Design

Historical product/design note from 2026-05-06. The useful idea is still current: KubeCove should feel like a calm operational workspace with topology and detail views, not a generic cluster dashboard.

## Intent

Primary user: an app developer or operator who wants to understand how an application exists inside Kubernetes without jumping between `kubectl`, dashboards, and raw YAML.

The design combines:

- calm IDE density
- mission-control confidence
- read-only Kubernetes inspection
- topology, table, and details as synchronized views of the same data

## Principles

- Topology is a primary surface, not a novelty view.
- Table and map represent the same selected scope.
- Context flows from left to right: scope, work surface, inspector.
- Namespace and app context stay visible.
- Kubernetes access stays Rust-side through typed Tauri commands.
- The frontend never receives raw kubeconfig data and never runs arbitrary shell commands.
- Argo CD, Helm, metrics, events, and logs enrich the browser when available.

## Layout

```text
+------------------+---------------------------+------------------+
| Context rail     | Table / topology surface  | Inspector        |
|                  |                           |                  |
| Cluster/context  | Resource table            | Summary          |
| Namespace scope  | Ownership/network map     | Details          |
| App/owner scope  | Health and filters        | YAML/events/logs |
+------------------+---------------------------+------------------+
```

The center can favor table, map, or split table/map depending on workspace mode and available data.

## Topology Lenses

### Ownership

Shows creation and ownership relationships:

- Deployment -> ReplicaSet -> Pod
- StatefulSet -> Pod and PVC
- DaemonSet -> Pod
- CronJob -> Job -> Pod
- Helm labels and Argo CD tracking metadata when owner references are not enough

### Network

Shows traffic-oriented relationships when enough data is available:

- Ingress or gateway -> Service
- Service -> EndpointSlice or selected Pods
- Pod -> container ports

Future network detail can include NetworkPolicy, service mesh data, and node placement.

## Table / Map Sync

- Selecting a map node selects the matching table row.
- Selecting a table row focuses the map node when present.
- Namespace, kind, health, app, and owner filters apply to both views.
- Sorting and pagination affect the table without changing the graph model.

## Inspector

The inspector opens for selected resources and remains read-only.

Core sections:

- summary and health
- metadata, labels, annotations, owner references, conditions, and status
- YAML
- events
- logs for pods and relevant workloads
- Argo CD or Helm context when available

The inspector never exposes credentials, tokens, or certificate data.

## Visual Direction

- Dense but breathable.
- High contrast without visual noise.
- Tables optimized for scanning.
- Topology optimized for relationships, not decoration.
- Clear active, selected, loading, empty, and error states.
- Original styling only; do not copy K8Studio, Aptakube, or Argo CD visuals.

## Phasing

1. Resource browser: context, namespaces, kind filters, table, read-only details/YAML.
2. Ownership map: workload ownership graph and table/map selection sync.
3. Network map: services, ingresses, endpoints, pods, and ports.
4. Incident polish: events, logs, health summaries, warning/restart shortcuts.
5. Optional enrichment: Argo CD, Helm, metrics, RBAC, and later guarded operational workflows.

## Non-Goals

- No cluster mutations in the default product path.
- No raw kubeconfig exposure.
- No arbitrary frontend shell execution.
- No copied competitor layouts or assets.
- No terminal-centric workflow as the primary UX.

## Open Questions

- How should large namespaces collapse or cluster graph nodes?
- Which topology lens should be default per workspace type?
- How much network inference should happen automatically versus through explicit user scope?
- When mutation features arrive, how should read-only and advanced modes be separated?
