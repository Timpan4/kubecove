# RBAC and permissions

**RBAC Cockpit** inspects Kubernetes Role-Based Access Control objects and can submit an explicit access review. Inventory, risk hints, observed grants, and authorization answers are different kinds of evidence; do not treat one as another.

## Inspect RBAC inventory

The cockpit reads five object families across the entire cluster:

- **Namespace Access**;
- **Roles**;
- **Cluster Roles**;
- **Bindings**;
- **Service Accounts**.

This inventory is cluster-wide regardless of workspace namespace filters. Your current Kubernetes identity still needs permission to list or get the objects.

1. Open **RBAC** and read **Coverage complete** or **Coverage partial**.
2. Search or filter by **All**, **High**, **Medium**, **Low**, **No flags**, or **Unknown**.
3. Select an object and inspect **Exact rules**, **Subjects**, and binding evidence.
4. For an identity, inspect **Observed RBAC grants** and its **Source chain**.
5. Select **Copy evidence** only after reviewing the output for sensitive identifiers.

Risk levels are policy heuristics that highlight broad verbs, resources, or binding patterns. They are not Kubernetes admission or authorization decisions. **Coverage complete** means the supported inventory requests completed; it does not prove that every external authorizer or authorization source is represented.

## Verify one exact action

Use **Verify access** when you need a live answer for an exact request. KubeCove submits a Kubernetes `SubjectAccessReview`; it never infers the answer from observed grants.

1. Select **Investigate identity** for the intended subject.
2. Under **Verify exact action**, choose either a resource request or non-resource request.
3. Enter the exact verb and target fields. For a resource request, review API group, resource, optional subresource, namespace, and name. For a non-resource request, review verb and path.
4. Submit **Verify access**.
5. Read the returned allowed, denied, or evaluation-error result and reason.

Verification checks the request you entered at that moment. It does not authorize a broader action and can become stale when policy changes.

## Understand observed grants

**Observed RBAC grants** are derived from Roles, ClusterRoles, RoleBindings, and ClusterRoleBindings that KubeCove loaded. **Source chain** shows which binding and role evidence produced an observation.

- **No observed grants for this identity** is not proof of denial.
- Partial inventory can omit a source chain.
- Aggregated roles, external authorizers, impersonation rules, admission controls, or other cluster mechanisms can affect the final outcome.
- Only explicit live verification answers the exact request, and even that answer does not execute it.

## Empty and error states

| Message | Response |
| --- | --- |
| **RBAC inspection unavailable** | Check context, connectivity, and list/get permissions for RBAC objects. |
| **Partial RBAC data** | Treat object counts, risk filters, and observed grants as incomplete. |
| **No matching RBAC objects.** | Clear search and risk filters. |
| **Select an RBAC object to inspect evidence.** | Choose one inventory row. |
| **No policy heuristic flags for this object.** | No supported risk rule matched; this is not a security approval. |
| **No observed grants for this identity.** | Verify the exact action rather than assuming denial. |
| **Unknown** | Evidence was insufficient for a stronger classification. |

Request least privilege for the exact namespace, resource, subresource, and verb needed. For mutation permissions used by KubeCove, see [Guarded Operations](https://github.com/Timpan4/kubecove/wiki/Guarded-Operations), [Pod Exec](https://github.com/Timpan4/kubecove/wiki/Pod-Exec), and [Port Forwarding](https://github.com/Timpan4/kubecove/wiki/Port-Forwarding).
