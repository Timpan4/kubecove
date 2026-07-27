# Glossary

## Application

Argo CD custom resource that declares desired delivery state and reports sync and health status.

## Context

Named kubeconfig entry selecting Kubernetes cluster, user, and namespace defaults.

## Custom Resource Definition (CRD)

Kubernetes extension defining custom resource API type. Custom Resource discovery requires CRD presence and permitted discovery/list access.

## Diagnostics

Optional bounded in-memory timing, counter, and trace collection for local investigation.

## Dry run

Server-side validation request that returns expected effect without persisting change. Guarded YAML workflows show dry-run diff before final confirmation.

## GitOps transport

Data path used for GitOps views: Kubernetes CRDs or explicit connected Argo CD API session. These transports are separate.

## Helm reconciliation

Read-only comparison between decoded Helm release intent and live Kubernetes objects. It reports tracked, missing, unlabeled, label-only, or unavailable evidence.

## Kubeconfig source

Ordered inputs used to discover contexts: selected environment variable plus application-added kubeconfig paths. Standard default discovery applies when configured paths are absent.

## Live session

Running port-forward or Pod exec session. It is not equivalent to workspace identity and can be stopped when leaving scope.

## Namespace scope

Workspace filter defining namespaces included in namespaced resource queries. It does not apply to cluster-scoped resources.

## Ownership map

Graph of resource ownership and related workload structure. Full-map mode retains unrelated branches during selection.

## Pod exec

Guarded terminal session targeting one selected Pod and optional container through Kubernetes `pods/exec` API.

## Port forward

Loopback-only local listener forwarding a selected Service target to Kubernetes workload endpoint.

## RBAC

Kubernetes role-based access control. KubeCove can only read or operate within permissions granted to selected credentials.

## Resource scope

Combination of selected context, namespace scope, resource kind, and resource identity used to bound query or operation.

## Secret redaction

Hiding Secret data values from normal details and YAML output. Reveal reads selected key through backend path; core `v1` Secret YAML apply remains blocked.

## Server-side apply force-conflicts

Option allowing guarded YAML operation to take ownership of fields owned by another field manager. Review target and dry-run diff before use.

## Tauri boundary

Typed desktop command boundary between webview and Rust backend. Kubeconfig contents, raw credentials, and generic local-shell access do not cross it.

## Workspace

Local working identity containing selected context and filters used to bound inspection and operations.
