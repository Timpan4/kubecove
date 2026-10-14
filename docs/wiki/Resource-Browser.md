# Resource browser

The Resource browser lists Kubernetes objects inside the workspace's saved contexts, namespaces, and kinds. Opening or filtering a row is read-only. Changes live behind the separate guarded-operation flows.

![KubeCove Kubernetes resource browser showing ownership map, resource health, and workload metrics](https://raw.githubusercontent.com/Timpan4/kubecove/main/docs/assets/resources-view.png)

## Find a resource

1. Open **Resources** from the workspace.
2. Use **Namespace** and **Kinds** to narrow the saved scope.
3. Search by name, namespace, kind, owner, GitOps owner, or Helm release.
4. Select a health summary or **All GitOps owners** when you need a narrower result.
5. Sort or group the table, then select a row to open its details.
6. Select **Clear** to remove active filters.

The table can show **Name**, **Namespace**, **Kind**, **Status**, **Ready**, **Restarts**, **CPU**, **Memory**, **Owner**, and **Age**. Available columns depend on the object and APIs returned by the cluster.

## Read the health summaries

| Filter | Meaning |
| --- | --- |
| **Total** | All objects collected for the current scope. |
| **Healthy** | Objects whose observed state matches KubeCove's healthy rules. |
| **Unknown** | Objects with a recognized health semantic that current evidence cannot classify. |
| **Not evaluated** | Objects for which KubeCove has no health semantic. |
| **Needs attention** | Objects with warning or incomplete signals that deserve inspection. |
| **Degraded** | Objects whose available condition or workload signals indicate degradation. |
| **Restart evidence** | Objects associated with container restart signals. Restarts are evidence, not a health state. |

These labels summarize available evidence. They are not Kubernetes authorization decisions, root-cause diagnoses, or proof that unlisted objects are healthy.

## Use table and map views

**Resource Table** is the quickest path to filtering and row details. You can sort results, collapse groups, page through larger result sets, and pin frequently inspected resources.

**Map** visualizes ownership relationships for the loaded objects. Use it to follow a workload toward its controllers or related objects. Metrics and topology are supplementary: either can be missing when the API, scope, or access does not provide enough data.

Use the status menu in the Resource browser header to check live-watch state and reload errors. Choose **Refresh (clear cache)** to clear cached data for this view and reload active reads; filters and selection are preserved. Health badges show their source and raw status in a tooltip on hover or keyboard focus. Partial-assessment badges remain visible.

Custom Resources appear when their discovery and loading setting is enabled. KubeCove discovers the kinds exposed by the selected cluster rather than assuming every cluster has the same APIs.

## Use resource actions

The **Actions** tab exposes a limited set of guarded operations for selected resources:

| Kind | Supported actions |
| --- | --- |
| Deployment | Change replicas, pause or resume, restart, roll back to an owned ReplicaSet revision, edit containers or metadata, delete |
| StatefulSet | Change replicas, restart, edit containers or metadata, delete |
| DaemonSet | Restart, edit containers or metadata, delete |
| Pod | Edit metadata, delete |
| ConfigMap | Edit metadata, delete |

Container edits target a regular container by name. They can change its image, literal environment variables, and resource requests or limits. Metadata edits change labels and annotations on the selected object, not a workload's Pod template or selector. Other kinds and Secret edits are not part of this Actions allowlist.

Actions remain visible when Kubernetes permission is denied or cannot be determined, but the unavailable action is locked and shows the required permission. You can collect several edits for one object and review them as one atomic change. By default, KubeCove sends a server-side dry-run and shows a diff between the current YAML and the API server's result before applying. If the resource UID or resource version changes after review, execution is rejected as stale. A failed dry-run cannot be applied.

In global **Settings > Advanced**, **Skip change review** skips the successful review screen only, after an explicit warning. Server-side dry-run still runs, and exact deletion always requires typing the resource name. The selected-resource YAML editor keeps its separate second **Apply** step.

## Understand loading and empty states

| Message | Meaning and next check |
| --- | --- |
| **No cluster context** | Choose an available context or edit the workspace. |
| **No resource scope** | Add usable contexts or kinds to the workspace. |
| **Failed to load resources** | Check cluster reachability, kubeconfig source, and list permissions. |
| **No resources match current scope and filters** | Clear filters or widen namespace and kind scope. |
| **Loading ownership map** | Resource rows may be ready while topology is still loading. |
| **Failed to load ownership map** | Select **Retry**; table inspection can remain available. |

Realtime watches refresh supported resource data while connected. A disconnected or restricted watch does not make the last rendered state an authoritative live view; refresh and compare with the Kubernetes API before acting.

Next: [inspect one resource](https://github.com/Timpan4/kubecove/wiki/Resource-Details) or [investigate incident signals](https://github.com/Timpan4/kubecove/wiki/Incidents).
