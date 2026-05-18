# Product Vision

## Identity

KubeCove is a local desktop Kubernetes workspace with an Aptakube-clean aesthetic. It is neither a minimal cluster dashboard nor an overloaded IDE. The product personality is **focused clarity under load**: present the right information quickly, support fast navigation between incident context and resource detail, and keep mental overhead low even when working across multiple clusters or namespaces.

Inspiration benchmarks are K8Studio and Aptakube. K8Studio demonstrates what a serious Kubernetes GUI can grow into. Aptakube demonstrates a clean, modern aesthetic that this project should approach without matching directly. The visual language is original and must not copy K8Studio or Aptakube code, assets, branding, layouts, or marketing text.

## Entry Point

The first screen after launch is a **saved workspace launcher**. Users restore work by selecting a named workspace rather than replaying an exact stale state. A workspace encodes which clusters, namespaces, and resource filters were active, but the **restore experience is a curated home overview**, not a full state replay. Curation means the app refreshes live cluster state for the saved scope, summarizes current health, marks missing saved resources as unavailable instead of replaying stale data, and surfaces useful entry points for the saved context. The overview shows cluster health, GitOps sync status when GitOps metadata is available, and quick-access shortcuts to the most-used namespaces and applications.

This preserves startup speed and avoids state mismatches from clusters that have changed since the workspace was saved.

## Inside a Workspace

A workspace presents a **hybrid IDE tree plus fast shortcuts**. The left sidebar shows a cluster/namespace/resource-kind tree for deep navigation. The main area shows a fast-filtering resource table with a health strip along the top. Fast shortcuts are workspace-scoped entry points: user-pinned scopes, recent namespaces or applications, common resource kind filters, and health-driven shortcuts such as Unhealthy, Recent, Compare, and GitOps Drift.

The split between table and detail panel is the primary working surface. The table answers "what is there." The detail panel answers "what is wrong with it."

## Core Jobs

Three operational jobs drive every feature decision:

1. **Browse resources fast.** List, filter, and search across namespaces and clusters with minimal friction.
2. **Troubleshoot incidents.** Get from symptom to resource detail, YAML, events, and health status in as few clicks as possible.
3. **Understand app topology.** See how workloads, services, and ingresses relate, and whether Argo CD is managing the application.

## Navigation Model

Namespace is first-class navigation state, not a filter buried in a dropdown. The navigation hierarchy is:

```
Cluster group -> Cluster/context -> Namespace -> App/owner -> Resource
```

This matches the mental model of operators working in a specific namespace across a deployment, rather than bouncing between random resource types.

Supported navigation patterns:
- **Namespace-first.** Start in a namespace and browse its resources.
- **Cluster-first.** Start at the cluster level and drill into namespaces.
- **App-first context.** When GitOps or ownership metadata is present, optional app entries appear in the tree, filters, and shortcuts so users can jump from application context back to Kubernetes resources.
- **Multi-namespace workspace.** A single workspace can span multiple namespaces across one or more clusters.

Namespace is never hidden or collapsed. It is always visible in the navigation state and table context.

## Multi-Cluster

Multi-cluster is a first-class concept, not an add-on. The app supports:
- **Aggregate views.** See resources from multiple clusters in a single table.
- **Side-by-side compare.** Open two clusters in split view for drift comparison.
- **Custom cluster groups.** Define named groups of contexts for quick switching.

Cluster credentials stay on the Rust side. The frontend receives only context metadata, not raw kubeconfig contents.

## Visual Emphasis

The default workspace layout uses a **split table plus health strip**. The table shows resource name, kind, namespace, status, age, and owner. A health strip at the top of the table summarizes healthy, degraded, and missing resources for the current filter context.

Later, **adaptive workspace defaults** can provide different default layouts for workspace types, such as resource browser, incident review, app map, or cluster comparison. This should be explicit and local to the saved workspace, not hidden behavior tracking. A custom density toggle will be added for users who prefer more or less information per screen.

## GitOps Integration Posture

GitOps is an **optional intelligence and extension layer**, not the product backbone. The core Kubernetes browser works with or without Argo CD. When Argo CD is present, it adds:

- **Resource overlay.** Resources managed by an Argo application show the application name, sync status, and revision.
- **Health and drift dashboard.** A read-only view summarizing Argo application health and sync drift across the workspace.
- **Filters.** Filter the resource table by Argo application, sync status, or revision.
- **App topology hints.** When an Argo application is selected, the table and detail view highlight related resources in the application chain.

Argo CD support uses the Kubernetes API, not the Argo CD CLI or API. The app detects Argo CRDs (`argoproj.io/v1alpha1` Application, ApplicationSet, AppProject) and surfaces their status through the same list and detail flows as native Kubernetes resources.

The language for GitOps tooling remains open. Argo CD is the first target, but future Flux and Fleet integrations should follow the same Kubernetes-API-first pattern when possible. No GitOps CLI, external API, sync, rollback, or mutation workflow should be added without an explicit ADR and the same read-only-by-default guardrails as the rest of the product.

Argo CD does not become the navigation backbone. The namespace-first tree and fast shortcuts remain primary.

## Safety Posture

The MVP is read-only. No create, update, delete, scale, sync, or rollback operations are exposed in the UI.

Future mutation capabilities are not forbidden, but they require:
- An explicit Architecture Decision Record describing the mutation workflow.
- Guardrails that make mutating actions deliberate, permission-gated, and reversible.
- A separate advanced mode that must be explicitly enabled, not a default path.

Visible disabled actions in the UI serve as placeholders and signals that advanced operations exist behind proper guardrails. This makes the mutation path discoverable without making it accessible.

## Density

The baseline is **balanced IDE density**: enough information visible to work efficiently without the claustrophobia of a terminal multiplexer or the emptiness of a minimal dashboard. The table shows the most important columns by default. Detail panels are readable and structured.

A **custom density toggle** is a planned enhancement, not the initial state. The default is medium density. Compact and expanded modes can be added later without changing the baseline experience.

## Near-Term Implications

The first milestones focus on making the browser genuinely useful:
- Fast, filterable resource tables across namespaces and clusters.
- Read-only YAML and detail views with owner references, labels, and status.
- Argo CD detection and basic read-only Argo views.
- Local saved workspaces that restore to a curated overview rather than stale exact state.

These produce a working Kubernetes IDE that answers "what is in my cluster" and "is it healthy" quickly and reliably.

## Long-Term Implications

Later milestones can add:
- Topology and relationship maps.
- Log streaming and event views.
- Helm release management.
- RBAC and security inspection.
- Metrics integration.
- Context-aware AI troubleshooting assistance.
- Local SQLite state for workspace history and saved bookmarks.

Each of these follows the same principles: namespace-first navigation, Rust-side Kubernetes access, read-only by default, and explicit ADRs for mutation workflows.
