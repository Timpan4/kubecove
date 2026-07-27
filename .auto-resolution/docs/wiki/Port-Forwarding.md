# Port Forwarding

Open a local TCP listener to one namespaced Pod or one selector-backed Service.

Related: [Resource Details](https://github.com/Timpan4/kubecove/wiki/Resource-Details) · [Workspaces and Scope](https://github.com/Timpan4/kubecove/wiki/Workspaces-and-Scope) · [RBAC and Permissions](https://github.com/Timpan4/kubecove/wiki/RBAC-and-Permissions)

## Prerequisites

- Select an exact namespaced **Pod** or **Service**, then open **Forward** in Resource Details.
- Use Kubernetes access authorized for `pods/portforward` in the namespace.
- Choose a TCP remote port from 1 through 65535. For a Service, choose one of its TCP Service ports when listed.
- Leave **Local port** empty for automatic selection, or choose an available local port from 1024 through 65535.

The listener binds only to `127.0.0.1`. It is not reachable from another host. A local port already used by a live session or another process cannot be used.

## Target and preview

The target card shows the namespace, resource name, and kind. **Start** is the explicit confirmation; there is no separate preview or confirmation checkbox.

For a Pod, the remote port forwards to that Pod directly.

For a Service, the app resolves the selected Service port to one ready, running Pod matched by its selector. A selectorless Service, `ExternalName` Service, Service without TCP ports, Service port that does not exist, or Service without a ready matching Pod cannot start. The session shows the resolved Pod and port.

A Service `targetPort` is resolved to its numeric target, named container port, or Service port. If a named `targetPort` is absent on the resolved Pod, correct the Service or Pod specification before retrying.

## Confirm, execute, and verify

1. Check the target and remote port.
2. Enter an optional local port, or leave it blank for an automatic local port.
3. Select **Start**.
4. After the session reports **listening**, copy or open the displayed local URL, such as `http://127.0.0.1:18080`.
5. Test the local endpoint, then inspect the active session's target, resolved Pod, local address, and status.

Each local connection opens a Kubernetes port-forward. Service sessions resolve a ready backing Pod again when a connection reconnects or when you select **Reconnect**; the resolved Pod can change. The session stays visible in Resource Details, **Live Sessions**, and the live-session indicator.

## Save Service presets

Only Service targets can be saved. In a Service's **Forward** tab, choose valid ports and select **Save preset**. The preset stores the workspace-scoped cluster context, namespace, Service name, Service port, optional local port, and optional label; it does not save a live session or a resolved Pod.

In **Live Sessions**, start, edit, or delete saved Service presets. **Start saved** checks active sessions and reports local-port conflicts instead of replacing another target.

When a workspace with saved Service presets is restored:

- With **Auto-start saved port forwards** off, choose **Start saved**, **Review**, or **Skip** in the restore prompt.
- With it on, the workspace starts new sessions from its saved Service presets once per restore. This does not restore a previous live session after an app restart.

Change this setting in **Settings** → **Live sessions** or in the Live Sessions view.

## Stop and scope cleanup

Select **Stop** from Resource Details, Live Sessions, or the live-session indicator to close the listener. Reconnect stops the old session before opening a new one.

By default, leaving a workspace or kubeconfig-source scope stops sessions outside the new scope. **Keep live sessions across workspace switches** changes that workspace-switch behavior. App exit stops all sessions; sessions are not persisted.

## Recover from failures

- **Target unsupported:** choose an exact namespaced Pod or selector-backed Service. Deployments, selectorless Services, and `ExternalName` Services are unsupported.
- **Service has no ready Pod:** wait for a ready running Pod that matches the Service selector, then start again.
- **Port failure:** choose a valid TCP remote port and an unused local port at least 1024, or leave the local port blank.
- **Forbidden:** request `pods/portforward` in the target namespace; RBAC remains authoritative.
- **Session error:** read the visible error and resolved target, inspect the Pod and Service, then use **Reconnect** or start a new session after correction.
