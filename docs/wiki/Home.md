# KubeCove

KubeCove is a fast, local Kubernetes desktop GUI for cluster inspection and troubleshooting. It gives Kubernetes-aware operators and application developers a visual resource browser for context, namespace, application ownership, resource state, events, logs, metrics, YAML, topology, GitOps, Helm, and RBAC before taking action.

Kubernetes credentials remain on the Rust side of the desktop boundary. KubeCove does not deploy cluster agents, expose raw kubeconfig contents to the frontend, or provide arbitrary shell execution. Operations are narrow, typed, target-scoped, and explicitly confirmed.

![KubeCove Kubernetes resource browser showing ownership map, resource health, and topology](https://raw.githubusercontent.com/Timpan4/kubecove/main/docs/assets/resources-view.png)

## Start

1. [Download latest installer](https://github.com/Timpan4/kubecove/releases/latest). Installers are beta builds; OS trust prompts may require approval.
2. Make a readable kubeconfig context available through `$KUBECONFIG` or your default kubeconfig location.
3. Create a workspace. Choose a context, then narrow namespaces and resource kinds when useful.
4. Open a resource. Inspect details, events, logs, metrics, YAML, ownership, and related GitOps or Helm metadata before using a guarded action.

## Safety boundary

Inspection is default. Guarded workflows include Pod and selector-backed Service port forwarding, exact-Pod exec, selected-resource YAML apply, narrow scale/restart/delete actions, and reviewed Argo CD refresh and sync operations.

YAML apply is limited to selected resources, validates identity, dry-runs before final confirmation, and blocks v1 Secret apply. Argo CD transport is an explicit choice: Kubernetes CRD browsing and a connected Argo CD API session never silently fall back between each other. Flux remains inspection-only.

## Start and navigate

- [Install and update KubeCove](https://github.com/Timpan4/kubecove/wiki/Install-and-Update)
- [Create a workspace and choose scope](https://github.com/Timpan4/kubecove/wiki/Workspaces-and-Scope)
- [Use the workspace overview](https://github.com/Timpan4/kubecove/wiki/Workspace-Overview)
- [Browse Kubernetes resources](https://github.com/Timpan4/kubecove/wiki/Resource-Browser)
- [Inspect resource details](https://github.com/Timpan4/kubecove/wiki/Resource-Details)

## Inspect

- [Investigate incidents](https://github.com/Timpan4/kubecove/wiki/Incidents)
- [Inspect Argo CD and Flux](https://github.com/Timpan4/kubecove/wiki/GitOps-Argo-CD-and-Flux)
- [Review Helm reconciliation](https://github.com/Timpan4/kubecove/wiki/Helm-Reconciliation)
- [Review RBAC and permissions](https://github.com/Timpan4/kubecove/wiki/RBAC-and-Permissions)

## Operate safely

- [Scale, restart, or delete a supported resource](https://github.com/Timpan4/kubecove/wiki/Guarded-Operations)
- [Edit and apply selected-resource YAML](https://github.com/Timpan4/kubecove/wiki/Edit-and-Apply-YAML)
- [Start a guarded Pod exec session](https://github.com/Timpan4/kubecove/wiki/Pod-Exec)
- [Start and manage a port forward](https://github.com/Timpan4/kubecove/wiki/Port-Forwarding)

## Configure and troubleshoot

- [Use Settings, updates, and diagnostics](https://github.com/Timpan4/kubecove/wiki/Settings-Updates-and-Diagnostics)
- [Understand safety, data handling, and architecture](https://github.com/Timpan4/kubecove/wiki/Safety-Data-and-Architecture)
- [Troubleshoot common symptoms](https://github.com/Timpan4/kubecove/wiki/Troubleshooting)
- [Look up KubeCove terms](https://github.com/Timpan4/kubecove/wiki/Glossary)

## Help and documentation

[Get support or report a product issue](https://github.com/Timpan4/kubecove/issues). [Read security policy and privately report vulnerabilities](https://github.com/Timpan4/kubecove/security/policy). [Open a documentation issue](https://github.com/Timpan4/kubecove/issues/new).

Maintainers own documentation source edits. Readers should report documentation gaps or errors through a documentation issue.
