# Architecture Blueprint

This page describes the current app shape and the direction for near-term work. File placement, size caps, and hygiene rules live in the [engineering handbook](handbook/).

## Product Shape

KubeCove is a local desktop Kubernetes workspace with context-first and namespace-first navigation.

```text
Cluster group -> Cluster/context -> Namespace -> App/owner -> Resource
```

A cluster group is local saved navigation metadata. A saved workspace stores contexts, namespaces, filters, shortcuts, and layout preference. Restoring a workspace fetches live cluster state and opens a curated overview for that scope.

The differentiator is persistent global scope across selected contexts, namespaces, resource kinds, health, owner references, GitOps signals, Helm releases, RBAC context, metrics, events, logs, topology, and guarded operations.

## Security Boundary

The Svelte frontend is an untrusted UI surface relative to the Rust backend.

- The frontend must not receive raw kubeconfig contents, tokens, or certificate data.
- The frontend must not run arbitrary shell commands.
- Kubernetes list, get, discovery, watch, events, logs, metrics, GitOps, Helm, RBAC, live sessions, and operation flows must cross typed Tauri commands.
- Normal Kubernetes API access uses `kube-rs`.
- Pod and selector-backed Service port-forwarding must follow [ADR 0003](decisions/0003-guarded-live-sessions.md). Other cluster-changing operations must follow [ADR 0004](decisions/0004-guarded-cluster-operations.md).
- Connected Argo CD access follows [ADR 0013](decisions/0013-argocd-connected-inspection-and-operations.md): explicit transport selection, backend-owned credentials and TLS, bounded responses, Rust-side Secret redaction, and preflighted allowlisted operations.

`kubectl`, Helm CLI, Argo CD CLI, and Flux CLI are not core data paths. Connected Argo CD uses its HTTP API directly; no command-line tool or automatic transport fallback is involved.

## Frontend

Current high-level layout:

```text
src/
  app/                  app shell, top bar, router, shared app frames
  components/           generic UI and shared display components
  features/
    app-updates/
    argo/
    command-palette/
    gitops/
    helm/
    incidents/
    live-sessions/
    rbac/
    resource-detail/
    resources/
    workspaces/
  lib/                  pure logic, stores, typed Tauri wrappers, types
```

Frontend rules:

- Components call typed wrappers in `src/lib/tauri.ts`, not raw `invoke()` directly.
- Feature-specific components stay in `src/features/<area>/`.
- Generic shadcn primitives and cross-feature display components stay in `src/components/`.
- Pure shared logic belongs in `src/lib/`.
- Frontend state should keep selected context, namespace scope, kind scope, active workspace, selected resource, active detail view, and future operation targets explicit.

## Backend

Current high-level layout:

```text
src-tauri/src/
  commands/
    argo/
    cancellation.rs
    flux/
    helm/
    pod_exec/
    resources/
    sessions/
    streams/
    contexts.rs
    diagnostics.rs
    discovery.rs
    events.rs
    incidents.rs
    metrics.rs
    namespaces.rs
    operations.rs
    rbac.rs
    usage.rs
    workspace_files.rs
  models/
    argo.rs
    cancellation.rs
    flux.rs
    cluster.rs
    diagnostics.rs
    discovery.rs
    error.rs
    events.rs
    helm.rs
    incidents.rs
    metrics.rs
    namespace.rs
    operations.rs
    rbac.rs
    resource.rs
    sessions.rs
    streams.rs
    usage.rs
```

Backend rules:

- `commands/` owns Tauri command handlers and command-domain helpers.
- `models/` owns serde contracts returned to the frontend.
- Commands return frontend-safe summaries, details, YAML strings, stream IDs, operation results, or typed error payloads.
- Raw Kubernetes objects should not become general frontend state. Details and YAML are explicit inspection surfaces.
- A separate `kube/` module can be introduced when Kubernetes access has a second backend consumer beyond commands.

## Command Families

Typed frontend wrappers currently cover:

- context commands: `list_kube_contexts`
- namespaces: `list_namespaces`
- discovery: `list_resource_kinds`
- resources: `list_resources`, `list_dynamic_resources`, `list_resource_scope`
- resource details: `get_resource_details`, `get_dynamic_resource_details`, `get_resource_yaml`
- resource revisions and Secret disclosure: `list_deployment_revisions`, `reveal_secret_data_value`
- guarded resource operations: selected-resource YAML prepare/apply/lint; scale, rollout-restart, and delete preview/run commands
- topology: `list_resource_topology`
- events and streams: `list_resource_events`, resource watches, event watches, pod log streams, `stop_stream`
- live sessions: `start_pod_port_forward`, `stop_port_forward`, `list_port_forwards`, guarded Pod exec session commands
- metrics: `list_resource_metrics`, app usage metrics
- GitOps: Argo CD CRD detect/list/detail commands; explicit connected Argo discovery, connection, inspection, comparison, preflight, and operation commands; Flux detect/list/detail commands for Source, Kustomize, Helm, Notification, and Image API resources
- Helm: release list, detail, and reconciliation commands
- RBAC: inspection summary and explicit access review
- workspace and runtime support: kubeconfig source management, workspace import/export dialogs, cancellation, diagnostics, and usage commands

The frontend GitOps section is provider-grouped. Its landing view uses Kubernetes detection and list commands to show provider cards, hides unavailable providers by default, and can show disabled provider placeholders when the global setting is enabled. Argo application details can use either Kubernetes CRD inspection or one explicitly selected connected profile. Flux remains inspection-only.

Every new command needs:

- a Rust serde model when the payload is structured
- a TypeScript mirror type
- a typed wrapper in `src/lib/tauri.ts`
- user-visible serialized errors
- no secret or broad filesystem leakage

Cluster-changing commands also need the ADR 0004 target, confirmation, and permission-aware UX contract unless a focused ADR defines a narrower model.

## Resource Strategy

Common resource kinds should stay typed or semi-typed where that provides better summaries. Custom Resources are discovered from `CustomResourceDefinition` objects, then listed through `DynamicObject` support. Namespace and GitOps resource scopes may append present CRD-backed kinds for the selected namespace set.

Core inspection surfaces:

- resource table summaries
- YAML
- metadata, labels, annotations, owner references, conditions, and status
- events
- pod logs
- topology
- metrics when `metrics.k8s.io` is available

GitOps CRDs remain a priority dynamic-resource area:

- `argoproj.io/v1alpha1` `Application`
- `argoproj.io/v1alpha1` `ApplicationSet`
- `argoproj.io/v1alpha1` `AppProject`
- `source.toolkit.fluxcd.io/v1` `GitRepository`, `OCIRepository`, `HelmRepository`, `HelmChart`, `Bucket`
- `kustomize.toolkit.fluxcd.io/v1` `Kustomization`
- `helm.toolkit.fluxcd.io/v2` `HelmRelease`
- `notification.toolkit.fluxcd.io/v1beta3` `Provider`, `Alert`
- `notification.toolkit.fluxcd.io/v1` `Receiver`
- `image.toolkit.fluxcd.io/v1` `ImageRepository`, `ImagePolicy`, `ImageUpdateAutomation`

## Extension Points

Future work can add deployment-aware port-forwarding, expanded exec scopes, richer Helm actions, guarded Flux operations, AI assistance, and durable local workspace history. CLI-backed integrations and security-sensitive additions require ADRs before implementation.

## Version Sources

The [README stack overview](../README.md#stack) records stable major versions. `package.json`, `src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json` are authoritative for exact dependency and app versions.

The tested Kubernetes window is 1.34–1.36 and is enforced by release and E2E workflows. Lab pins live in `e2e/harness/platform.ts`; docs record compatibility boundaries, not a duplicate patch-version inventory.
