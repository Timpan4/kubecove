# Resource details

Select a Resource browser row to open details for one exact Kubernetes object. Available tabs depend on kind, cluster APIs, and access.

## Read the Details tab

**Details** combines the object's current summary with the evidence KubeCove can read:

- status, readiness, restart, and owner badges;
- **Incident summary** and **Timeline** from available status and event signals;
- Kubernetes **Conditions**;
- labels, annotations, identity, and other **Metadata**;
- **Containers** and their observed states for supported workloads;
- **Advanced metadata** and structured status details.

**No active incident signals** means none were found for this object from available evidence. It is not a complete forensic conclusion. Missing events, conditions, or fields can mean the API did not return them or the current identity cannot read them.

## Choose the right tab

| Tab | Use it for |
| --- | --- |
| **Events** | Review Kubernetes events associated with the exact object. Events expire and may be unavailable. |
| **Logs** | Read supported Pod or container logs. Choose the exact container and relevant time range. |
| **Exec** | Start a guarded terminal session to an exact Pod and container. |
| **Forward** | Start a localhost port forward for a supported Pod or selector-backed Service. |
| **Revisions** | Inspect available Deployment revision history and related ReplicaSets. |
| **Argo CD** | Inspect Argo ownership or Application data when available. |
| **Actions** | Preview and confirm supported scale, restart, or delete operations. |
| **YAML** | Read rendered object YAML or enter the guarded edit-and-apply flow. |

Not every tab appears for every kind. For example, Exec and logs need an appropriate Pod/container target; revisions apply to supported workload history; guarded actions support only their documented kind matrix.

## Inspect YAML and Secrets safely

The YAML tab can switch between Kubernetes-style and apply-clean shapes, and between supported encodings. Apply-clean removes server-managed fields for a safer editing base; it is not a copy of the original manifest source.

Secret values remain redacted by default. Any available per-key reveal is transient and limited to the current view. Do not paste revealed values into issues, screenshots, chat, or diagnostics. Connected Argo Secret values remain masked before crossing the backend boundary.

## Follow ownership before changing live state

Use owner metadata, the ownership map, GitOps indicators, and Helm release information to identify a source of truth. A direct Kubernetes action changes the live object; it does not update a Git repository, Argo Application source, or Helm values. A controller can reconcile the direct change away.

For supported mutations, follow [Guarded Operations](https://github.com/Timpan4/kubecove/wiki/Guarded-Operations) or [Edit and apply YAML](https://github.com/Timpan4/kubecove/wiki/Edit-and-Apply-YAML).

## Resolve incomplete details

- **Loading details**: wait for the exact-object request to finish.
- **Failed to load details**: confirm the object still exists and the current identity can get it.
- **Status loaded; event signals are unavailable**: inspect status, then verify event access separately.
- **None detected** or **None**: no value was found in available data; this is not proof the cluster has no related evidence.
- **Loading Exec** or **Failed to load Exec**: verify Pod/container availability and exec permission, then select **Retry**.

For a broader symptom workflow, open [Troubleshooting](https://github.com/Timpan4/kubecove/wiki/Troubleshooting).
