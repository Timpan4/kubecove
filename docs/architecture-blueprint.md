# Architecture Blueprint

This page describes the current shape of the app and the intended direction for near-term work. The rules for file placement, size caps, and hygiene live in the [engineering handbook](handbook/).

## Product Shape

KubeCove is a local desktop Kubernetes workspace with context-first and namespace-first navigation.

```text
Cluster group -> Cluster/context -> Namespace -> App/owner -> Resource
```

A cluster group is local saved navigation metadata. A saved workspace stores contexts, namespaces, filters, shortcuts, and layout preference. Restoring a workspace fetches live cluster state and opens a curated overview for that scope.

The long-term differentiator is persistent global filtering across selected contexts, namespaces, resource kinds, health, owner references, Argo CD signals, Helm releases, RBAC context, metrics, events, logs, and topology.

## Security Boundary

The React frontend is an untrusted UI surface relative to the Rust backend.

- React must not receive raw kubeconfig contents, tokens, or certificate data.
- React must not run arbitrary shell commands.
- Kubernetes list, get, discovery, watch, events, logs, metrics, Argo, Helm, and RBAC reads flow through typed Tauri commands.
- Normal Kubernetes API access uses `kube-rs`.
- Mutating cluster operations require a new ADR and permission-aware UX.

`kubectl`, Helm CLI, and Argo CD CLI may become optional future sidecars or fallbacks. They are not the core data path.

## Frontend

Current high-level layout:

```text
src/
  app/                  app shell, top bar, router, shared app frames
  components/           generic UI and shared display components
  features/
    app-updates/
    argo/
    helm/
    rbac/
    resource-detail/
    resources/
    settings/
    workspaces/
  hooks/                generic React hooks
  lib/                  pure logic, stores, typed Tauri wrappers, types
```

Frontend rules:

- Components call typed wrappers in `src/lib/tauri.ts`, not raw `invoke()` directly.
- Feature-specific components stay in `src/features/<area>/`.
- Generic shadcn primitives and cross-feature display components stay in `src/components/`.
- Pure shared logic belongs in `src/lib/`.
- Frontend state should keep selected context, namespace scope, kind scope, active workspace, selected resource, and active detail view explicit.

## Backend

Current high-level layout:

```text
src-tauri/src/
  commands/
    argo/
    resources/
    streams/
    contexts.rs
    discovery.rs
    events.rs
    helm.rs
    metrics.rs
    namespaces.rs
    rbac.rs
    usage.rs
  models/
    argo.rs
    cluster.rs
    discovery.rs
    error.rs
    events.rs
    helm.rs
    metrics.rs
    namespace.rs
    rbac.rs
    resource.rs
    streams.rs
    usage.rs
```

Backend rules:

- `commands/` owns Tauri command handlers and command-domain helpers.
- `models/` owns serde contracts returned to the frontend.
- Commands return frontend-safe summaries, details, YAML strings, stream IDs, or typed error payloads.
- Raw Kubernetes objects should not become general frontend state. Details and YAML are explicit read-only surfaces.
- A separate `kube/` module can be introduced when Kubernetes access has a second backend consumer beyond commands.

## Command Families

Typed frontend wrappers currently cover:

- context commands - `list_kube_contexts`
- namespaces: `list_namespaces`
- discovery: `list_resource_kinds`
- resources: `list_resources`, `list_dynamic_resources`, `list_resource_scope`
- resource details: `get_resource_details`, `get_dynamic_resource_details`, `get_resource_yaml`
- topology: `list_resource_topology`
- events and streams: `list_resource_events`, resource watches, event watches, pod log streams, `stop_stream`
- metrics: `list_resource_metrics`, app usage metrics
- Argo CD: detect, list, and detail commands for Applications, ApplicationSets, and AppProjects
- Helm: release list and detail commands
- RBAC: read-only inspection summary

Every new command needs:

- a Rust serde model when the payload is structured
- a TypeScript mirror type
- a typed wrapper in `src/lib/tauri.ts`
- user-visible serialized errors
- no secret or broad filesystem leakage

## Resource Strategy

Common resource kinds should stay typed or semi-typed where that provides better summaries. Dynamic resources and CRDs should flow through Kubernetes discovery and `DynamicObject` support.

Core read-only surfaces:

- resource table summaries
- read-only YAML
- metadata, labels, annotations, owner references, conditions, and status
- events
- pod logs
- topology
- metrics when `metrics.k8s.io` is available

Argo CD CRDs remain a priority dynamic-resource area:

- `argoproj.io/v1alpha1` `Application`
- `argoproj.io/v1alpha1` `ApplicationSet`
- `argoproj.io/v1alpha1` `AppProject`

## Extension Points

Future work can add guarded YAML edit/apply, port-forward, pod exec, richer Helm actions, Argo CD API flows, AI assistance, and durable local workspace history. Security-sensitive additions require ADRs before implementation.
