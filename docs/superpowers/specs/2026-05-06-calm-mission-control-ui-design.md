# Calm Mission Control UI Design

## 1. Product Intent

Calm Mission Control is a local desktop Kubernetes IDE for app developers. It surfaces how an application exists and connects inside Kubernetes, not just raw object tables.

Primary user: app developer who wants to understand their app's runtime topology, ownership chains, and network paths without wrestling with kubectl or generic cluster dashboards.

The UI blends Calm IDE (refined, focused, dense-but-breathable) with Mission Control (bold topology, live status, operational confidence). Read-only MVP only.

## 2. Design Principles

- **Topology first**: the tree/canvas showing Deployment -> ReplicaSet -> Pod is the primary product surface, not an afterthought.
- **Two equal peers**: dense resource table and topology canvas are co-equal views of the same cluster data. User switches between them, not chooses one over the other.
- **Context flows left to right**: namespace/app scope on the left rail, object list or canvas in the center, inspector panel on the right.
- **Never expose raw kubeconfig**: frontend calls only typed Tauri command wrappers. Kubernetes access owned by Rust.
- **No arbitrary shell commands**: frontend cannot execute kubectl or other tooling directly.
- **Argo CD enrichment is optional**: when Argo CD CRDs are present, show app boundary, sync status, health, and drift/orphan hints. Not a requirement.

## 3. Information Architecture

```
+------------------+----------------------------------+----------------+
| Context Rail     | Center Workspace (split or focus) | Inspector      |
| (left, narrow)  | (expandable)                      | (right, narrow)|
+------------------+----------------------------------+----------------+
| Cluster selector | [Focus: Map+Table | Table | Map] | Selected obj   |
| Namespace scope  | Canvas (topology) + Table (list)  | YAML / Details |
| App/Workload     | or Table Focus or Map Focus       | Events / etc.  |
| filter           |                                  | (read-only)    |
+------------------+----------------------------------+----------------+
```

- **Context Rail (left)**: cluster selector, namespace picker, optional app/workload filter. Collapsible.
- **Center (Workspace)**: default split view showing canvas (topology) and resource table side by side. Three focus modes: Map+Table (split, default), Table Focus (table fills center), Map Focus (canvas fills center). Both views read from the same Kubernetes data via Tauri commands.
- **Inspector (right)**: shows details for the currently selected object in either view. YAML, status, events, container ports.

## 4. Main Layout

```
,---[Context Rail]---.----------[Center Canvas/Table]----------.---[Inspector]---.
|                    |                                        |                |
| [Cluster Selector] | [Lens Toggle: Network Flow | Ownership]| [Object Name]  |
|                    |                                        | [Kind]         |
| [Namespace]        |    +-----------+    +--------+         | [Namespace]    |
|   default          |    | Service A |--->| Pod 1  |         |                |
|   kube-system      |    +-----------+    +--------+         | [YAML]         |
|                    |          |                               | [Events]       |
| [Workloads]        |          v                               | [Ports]        |
|   deployment/app  |    +-----------+                         |                |
|                    |    | Endpoint  |                         |                |
+-------------------+----+-----------+-------------------------+----------------+
```

## 5. Tree Canvas Lenses

The canvas renders a directed graph of Kubernetes objects. Two mutually exclusive lenses drive what edges are shown:

| Lens | Purpose | Edges |
|------|---------|-------|
| **Network Flow** | How traffic flows through the cluster | Ingress/Gateway -> Service -> EndpointSlice/selector -> Pod -> container ports; later: NetworkPolicy/mesh/node placement |
| **Ownership Flow** | How objects were created and who owns them | Deployment -> ReplicaSet -> Pod; StatefulSet -> Pod -> PVC; CronJob -> Job -> Pod; edges from ownerReferences, Helm labels, Argo app labels |

Lens selector is a toggle button row at the top of the center canvas. Selected lens persists per session.

## 6. Network Flow Lens

Renders networking topology for the selected namespace/app.

**Nodes shown:**
- Ingress / Gateway (Istio, nginx, etc.)
- Service (ClusterIP, NodePort, LoadBalancer)
- EndpointSlice (associated with a Service)
- Pod (with container port mappings)

**Edges:**
- Ingress -> Service (based on ingress backend or hostname rules)
- Service -> EndpointSlice (via service.spec.selector)
- EndpointSlice -> Pod (via endpoint addresses)
- Pod -> container ports (displayed as port badges on the Pod node)

**Node detail (on select):**
- Service: exposed ports, selector labels, type
- Pod: container image, port mappings, node affinity

**Future edge candidates:**
- NetworkPolicy (ingress/egress rules between pods)
- ServiceMesh sidecar placement
- Node affinity/placement

## 7. Ownership Flow Lens

Renders creation/ownership topology for the selected namespace/app.

**Nodes shown:**
- Workload controllers: Deployment, StatefulSet, DaemonSet, CronJob, Job
- Intermediate controllers: ReplicaSet, Job (for CronJob)
- Leaf objects: Pod, PersistentVolumeClaim

**Edges:**
- Deployment -> ReplicaSet (ownerReferences)
- ReplicaSet -> Pod (ownerReferences)
- StatefulSet -> Pod (ownerReferences)
- StatefulSet -> PVC (PVC controller creates PVCs; display as dashed edge)
- CronJob -> Job (ownerReferences or label selector)
- Job -> Pod (ownerReferences)

**Ownership signals:**
- `ownerReferences` (primary)
- Helm labels: `app.kubernetes.io/managed-by=Helm`, `app.kubernetes.io/instance`, `helm.sh/chart` (fallback when ownerRefs absent)
- Argo CD Application ownership via `argocd.argoproj.io/instance` label or Argo CRD application metadata when available

**Node detail (on select):**
- Deployment/ReplicaSet: replica count, selector, strategy
- Pod: image, node selector, tolerations, resource requests/limits

## 8. Table/Canvas Sync

Both the canvas and the resource table share the same data query.

- Selecting a node in the canvas highlights the corresponding row in the table.
- Clicking a row in the table focuses the node in the canvas and pans to it.
- Namespace filter in the context rail applies to both views simultaneously.
- Sorting in the table (by name, kind, age) does not affect canvas layout.

## 9. Inspector

Opens when any object (node or table row) is selected.

**Sections:**
- **Header**: Kind, name, namespace, Uid
- **YAML tab**: Full object manifest (read-only, syntax highlighted)
- **Details tab**: Structured fields relevant to the active lens (ports for Network Flow, owner info for Ownership Flow)
- **Events tab**: Kubernetes events for this object (loaded through typed Tauri wrapper when event fetching is supported; read-only detail data)
- **Health tab**: Only when Argo CD is present; shows Application sync status, health, and drift if the selected object belongs to an Argo App

The inspector never exposes raw credentials, tokens, or certificate data.

## 10. Visual Direction

- **Color palette**: Dark neutral background (slate/charcoal), accent color for selection and active state (teal or blue), muted colors for secondary information.
- **Typography**: Sans-serif, monospace for YAML and resource names. High information density with clear hierarchy.
- **Canvas**: Directed graph with nodes as rounded rectangles/cards. Edges as subtle lines with directional arrows. Selected node has accent border.
- **Density**: Compact but not cramped. More like Calm IDE than a corporate dashboard.
- **K8Studio**: Used as a public benchmark for Kubernetes IDE capabilities. This design does not copy K8Studio code, assets, branding, or proprietary layouts. The differentiation is the two-lens topology view and context-first workflow.
- **Argo UI**: Argo CD's canvas/tree is inspiration for the ownership lens, but the visual language and implementation are distinct.

## 11. Empty, Error, and Loading States

- **No namespace selected**: Context rail shows "Select a namespace" placeholder. Center shows instructional empty state.
- **No objects in namespace**: Canvas shows dashed border with "No objects found" message. Table shows empty state with filter suggestion.
- **Loading**: Skeleton nodes animate on the canvas. Table rows show loading shimmer.
- **Error fetching resources**: Toast notification with error message. Canvas shows error boundary with retry button.
- **Cluster unreachable**: Banner at top of context rail with disconnect indicator. Other controls remain accessible for reconnection.

## 12. MVP Phasing

**Phase 1:**
- Context rail: cluster selector, namespace picker
- Resource table with basic columns (Name, Kind, Namespace, Age)
- Read-only inspector with YAML tab
- Single lens: Ownership Flow only

**Phase 2:**
- Lens toggle row (Network Flow / Ownership Flow)
- Canvas rendering for Ownership Flow (Deployment -> ReplicaSet -> Pod)
- Table/canvas sync (select in table -> highlight in canvas)

**Phase 3:**
- Network Flow lens: Service -> EndpointSlice -> Pod
- Inspector detail tabs (Events, Health)
- Argo CD enrichment (health, sync status, app boundary) when CRDs present

**Out of scope for MVP:**
- Multi-cluster view
- YAML edit / apply
- Terminal or kubectl access
- Network policy visualization
- Service mesh sidecar injection display

## 13. Non-Goals

- This is not Argo CD. Argo CD is one of many possible Kubernetes tools this IDE could visualize.
- No raw kubeconfig exposure to frontend.
- No arbitrary shell command execution from frontend.
- Not a replacement for `kubectl get` or `kubectl describe`.
- Not a multi-cluster dashboard (MVP focuses on single-context view).

## 14. Open Questions

1. Should the context rail show a list of all workloads (Deployment, StatefulSet, etc.) for quick filtering, or just namespace/cluster selection?
2. Does the resource table default to showing all object kinds or just workload-related kinds?
3. How should the canvas handle large namespaces (50+ pods)? Should there be a collapse/zoom-out mode?
4. Should Network Flow lens attempt to render Ingress-to-Service edges based on ingress rules, or require the user to manually associate them?
5. Is there a preference for canvas layout algorithm (hierarchical top-down vs. force-directed)?