# Milestone 1 Completion Design

Historical design note from 2026-05-06. It describes the first working read-only Kubernetes browser. Current architecture and behavior are tracked in [Architecture Blueprint](../../architecture-blueprint.md) and [Milestones](../../milestones.md).

## Goal

Turn the scaffolded Tauri frontend app into a working read-only browser:

- choose a local kube context
- list namespaces
- filter namespaces and resource kinds
- show resources in a table
- open a read-only detail panel with structured details and YAML

## Scope

Frontend:

- one root dashboard route
- TanStack Query for server state
- TanStack Router for routing foundation
- TanStack Table for the resource grid
- local state for selected context, namespaces, kinds, and selected resource
- sidebar, resource table, and right detail panel

Backend:

- extend resource listing, details, and YAML support for common resource kinds
- keep all Kubernetes access behind existing Tauri commands
- keep kubeconfig and credentials out of frontend payloads

## Non-Goals

- mutations
- watches
- logs
- events
- Argo CD views
- Helm views
- multi-cluster aggregate views
- persistent workspaces

Those later became separate milestones.

## Data Flow

```text
Frontend dashboard
  -> list_kube_contexts()
  -> list_namespaces(context)
  -> list_resources(context, namespace, kind)
  -> get_resource_details(context, kind, namespace, name)
  -> get_resource_yaml(context, kind, namespace, name)
Rust/Tauri
  -> kube-rs client
  -> frontend-safe serde models or read-only YAML
```

Errors return as application errors that the UI can display without exposing kubeconfig, token, or certificate material.

## Resource Coverage

The milestone targeted common namespaced resources:

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

Cluster-scoped and dynamic discovery-backed resources were planned for later work.

## UI Shape

```text
+------------------+----------------------+------------------+
| Sidebar          | Resource table       | Detail panel     |
|                  |                      |                  |
| Context selector | Name / Namespace     | Details tab      |
| Namespaces       | Kind / Age           | YAML tab         |
| Kind filter      |                      | Read-only        |
+------------------+----------------------+------------------+
```

The table owns browsing. The detail panel owns inspection. The YAML view is read-only.

## Verification

Original completion checks:

- `bun run typecheck`
- `cargo check --manifest-path src-tauri/Cargo.toml`
- app launches in dev mode
- selecting context and namespaces loads resource rows
- selecting a row opens details and YAML
- namespace and kind filters affect the table

## Outcome

Milestone 1 was completed and later expanded by usability, Argo CD, discovery, workspace, incident, metrics, Helm, RBAC, and topology milestones.
