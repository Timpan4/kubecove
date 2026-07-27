# Troubleshooting

Start with selected context, namespace scope, and resource kind. Do not widen scope or disable protections merely to make result appear.

| Symptom | Safe action |
| --- | --- |
| Expected context is missing | Open **Settings > Kubeconfig**. Check selected environment-variable name, added paths, and load warnings. Confirm shell environment, then restart app after changing environment. |
| Wrong cluster or namespace data appears | Check workspace context and namespace scope before refresh. Recreate workspace scope if identity is unclear. |
| Resource is absent | Clear resource-kind filters, confirm namespace scope, then check context. Cluster-scoped resources do not belong to namespace filter. |
| Custom resource is absent | Keep **Settings > General > Custom Resources** enabled. Confirm CRD exists and account can discover and list it. |
| RBAC error or partial list | Use [RBAC and permissions](https://github.com/Timpan4/kubecove/wiki/RBAC-and-Permissions) to inspect effective access. Request least-privilege `get`, `list`, or `watch` permission for required resource and scope. |
| Metrics are empty | Confirm metrics API is installed, target resource emits metrics, and RBAC permits metrics access. Treat unavailable metrics as missing observability, not zero usage. |
| Logs stop or do not start | Confirm selected Pod and container still exist. Check Pod phase, container restart, namespace scope, and log RBAC. Restart stream after correcting target. |
| YAML looks incomplete or cannot apply | Select **Kubectl view** for full inspection. Check YAML error lens, identity, target scope, and dry-run diff. Core `v1` Secret apply is intentionally blocked. |
| Guarded operation is blocked | Re-read target, required confirmation text, supported kind, and RBAC. Use [Guarded operations](https://github.com/Timpan4/kubecove/wiki/Guarded-Operations) for supported actions; do not bypass confirmation. |
| Pod exec cannot connect | Confirm exact Pod, container, command, and `pods/exec` authorization. Check container shell exists. Stop stale session and start a new exact-target session. |
| Port forward fails or local port is busy | Verify Service target and selected port. Choose unused local port, then retry. Forward binds loopback only; test from same machine. |
| Argo CD application data is missing | Confirm Argo CRDs for Kubernetes transport, or connect server from Application details for API transport. Check URL, credential, TLS setting, and application RBAC. |
| Helm reconciliation is unavailable | Confirm Helm release metadata is readable and required Kubernetes discovery/list permissions exist. `unavailable` means verification could not establish clear result. |
| Update check or install fails | Confirm network access to GitHub release downloads, retry check, and relaunch only after installation completes. Development builds do not check updates. |
| Diagnostics report is empty or needs sharing | Enable diagnostics, reproduce once, refresh report, copy redacted output, and clear after capture. Include identifiers only for trusted recipient. See [Settings, updates, and diagnostics](https://github.com/Timpan4/kubecove/wiki/Settings-Updates-and-Diagnostics). |

If symptom persists, collect sanitized error text, selected context and scope, resource kind, and exact action. Report it through [documentation or product issue](https://github.com/Timpan4/kubecove/issues/new). Do not attach kubeconfig, tokens, Secret values, or unreviewed diagnostics output.
