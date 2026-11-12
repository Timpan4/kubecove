# Glossary

## Application

An Argo CD custom resource that declares a desired delivery state and reports sync and health status.

## Cluster group

One or more extra contexts saved with a workspace. The Resource browser reads them together with the primary context. It is local navigation data, not a Kubernetes object.

## Context

A named kubeconfig entry that selects a cluster, a user, and a default namespace.

## Custom Resource Definition (CRD)

A Kubernetes extension that defines a custom resource type. KubeCove lists custom resources only when the CRD exists and your account can discover and list it.

## Diagnostics

Optional timing, counter, and trace collection for local investigation. The data is bounded and held in memory.

## Dry run

A server-side request that validates a change and returns its result without saving it. Guarded YAML and Actions flows show a dry-run diff before the final step.

## GitOps transport

The data path a GitOps view uses: Kubernetes CRDs, or a connected Argo CD API session. The two are never mixed in one result.

## Helm reconciliation

A read-only comparison between the manifest stored with a Helm release and the live Kubernetes objects. Results are tracked, unlabeled live, missing, label-only, or unavailable.

## Kubeconfig source

The ordered inputs KubeCove reads contexts from: the selected environment variable, or standard `KUBECONFIG` when set and the platform-default kubeconfig otherwise, followed by kubeconfig paths added in Settings.

## Live session

A running port forward or Pod exec session. It belongs to the scope it was started in and stops when you leave that scope, unless the setting to keep live sessions is on.

## Namespace scope

The workspace filter that limits namespaced resource requests. It does not apply to cluster-scoped resources.

## Ownership map

A graph of ownership and related workload structure for the loaded objects. Full-map mode keeps unrelated branches visible while an object is selected.

## Pod exec

A guarded terminal session in one selected Pod and container through the Kubernetes `pods/exec` API.

## Port forward

A loopback-only local listener that forwards connections to one Pod, or to a ready Pod behind one selector-backed Service.

## RBAC

Kubernetes role-based access control. KubeCove can read and change only what the selected credentials are permitted to.

## Resource scope

The contexts, namespaces, and kinds that bound a request, plus the resource identity that bounds an operation.

## Secret redaction

Secret values are hidden in details and YAML. A reveal reads one selected key through the backend. Applying YAML for a core `v1` Secret stays blocked.

## Server-side apply force-conflicts

An option that lets a guarded YAML apply take ownership of fields owned by another field manager. Review the target and the dry-run diff before using it.

## Tauri boundary

The typed command boundary between the desktop webview and the Rust backend. Kubeconfig contents, raw credentials, and general shell access do not cross it.

## Workspace

A locally saved inspection scope: a primary context, an optional cluster group, namespaces, kinds, shortcuts, and saved Service port-forward presets.
