# Troubleshooting

Start with the selected context, namespace scope, and resource kind. Do not widen the scope or turn off a protection only to make a result appear.

| Symptom | What to do |
| --- | --- |
| An expected context is missing | Open **Settings > Kubeconfig**. Check the selected environment-variable name, the added paths, and any load warnings. If you changed the environment, restart the app. |
| Data from the wrong cluster or namespace appears | Check the workspace's context and namespace scope before refreshing. If the scope is unclear, edit the workspace or create a new one. |
| A resource is absent | Clear the kind filters, confirm the namespace scope, then check the context. Namespace filters do not apply to cluster-scoped resources. |
| A custom resource is absent | Keep **Settings > General > Custom Resources** on. Confirm that the CRD exists and that your account can discover and list it. |
| An RBAC error or a partial list appears | Use [RBAC and permissions](https://github.com/Timpan4/kubecove/wiki/RBAC-and-Permissions) to inspect access. Request `get`, `list`, or `watch` for the resource and scope you need. |
| Metrics are empty | Confirm that the metrics API is installed, that the resource reports metrics, and that RBAC permits reading them. Missing metrics do not mean zero usage. |
| Logs stop or do not start | Confirm that the Pod and container still exist. Check the Pod phase, container restarts, namespace scope, and log permissions, then restart the stream. |
| YAML looks incomplete or cannot be applied | Select **Kubectl view** to inspect the full object. Check the YAML error lens, the resource identity, the target scope, and the dry-run diff. Applying a core `v1` Secret is blocked on purpose. |
| A guarded operation is blocked | Read the target, the confirmation text, the supported kinds, and the required permission again. See [Guarded operations](https://github.com/Timpan4/kubecove/wiki/Guarded-Operations). |
| Pod exec cannot connect | Confirm the Pod, container, command, and `pods/exec` permission. Check that the container has the shell you chose. Stop the stale session and start a new one. |
| A port forward fails or the local port is busy | Check the target and the selected port. Choose an unused local port and retry. The forward listens on loopback only, so test it from the same machine. |
| Argo CD Application data is missing | For the Kubernetes transport, confirm that the Argo CD CRDs are installed. For the connected transport, connect a server from **Argo CD connection** in the GitOps view, then check the URL, credential, TLS settings, and Argo CD RBAC. |
| Helm reconciliation is unavailable | Confirm that Helm release storage is readable and that you can discover and list the resources the release references. **Unavailable** means the comparison could not be completed. |
| An update check or install fails | Confirm network access to GitHub release downloads and check again. Relaunch only after the install finishes. Development builds do not check for updates. |
| The diagnostics report is empty | Turn on diagnostics, reproduce the problem once, and refresh the report. Copy the redacted output, then clear the report. Include identifiers only for a recipient you trust. See [Settings, updates, and diagnostics](https://github.com/Timpan4/kubecove/wiki/Settings-Updates-and-Diagnostics). |

If the symptom persists, collect the sanitized error text, the context and scope, the resource kind, and the exact action. Report a product problem in [GitHub Issues](https://github.com/Timpan4/kubecove/issues) and a documentation problem with the [documentation issue form](https://github.com/Timpan4/kubecove/issues/new?template=documentation.yml). Do not attach kubeconfig files, tokens, Secret values, or diagnostics output you have not reviewed.
