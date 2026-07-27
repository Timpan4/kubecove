# Pod Exec

Start an interactive process only from the detail page of one namespaced Pod.

Related: [Resource Details](https://github.com/Timpan4/kubecove/wiki/Resource-Details) · [Port Forwarding](https://github.com/Timpan4/kubecove/wiki/Port-Forwarding) · [RBAC and Permissions](https://github.com/Timpan4/kubecove/wiki/RBAC-and-Permissions)

## Prerequisites

- Select the exact Pod in a namespace, then open **Exec** in Resource Details.
- Use Kubernetes access authorized for `pods/exec` in that namespace.
- Treat the session as direct container access. Confirm the context, namespace, Pod, container, and command before starting.

Exec does not start from a Service, Deployment, Job, owner, or selector.

## Target and command

Choose a container or leave **Default container** selected so Kubernetes chooses it. The target card shows the exact context, namespace, Pod, and container before execution.

Choose one command mode:

- **`/bin/sh`** starts exactly `['/bin/sh']`.
- **`/bin/bash`** starts exactly `['/bin/bash']`.
- **Custom argv** sends one argument per line, without local shell parsing.

For example, enter this custom argv:

```text
/usr/bin/env
printenv
```

This sends `['/usr/bin/env', 'printenv']`; it does not run through a local shell. Use an explicit shell and arguments when the container requires shell syntax.

## Confirm and execute

1. Read the target and command shown in the launch form.
2. Select **I understand this opens an interactive session in the selected Pod.**
3. Select **Start**.

The confirmation binds the displayed target and argv. Changing the container, command preset, or custom argv clears it. A new start replaces the terminal's active session after stopping the prior one.

## Use the terminal

The terminal streams process output and sends keyboard input to Kubernetes. Resizing the terminal sends the new terminal dimensions to the session. Session status is visible in the Pod detail and the Live Sessions indicator; use **Stop** there or in the active-session list to end it.

Output is kept only in the active terminal view. There is no transcript or session restoration after an app restart. **Clear terminal** clears the displayed terminal only; it does not alter the process or create a record.

## Verify and recover

- **Shell not found:** select the shell the image provides or use Custom argv with an executable path.
- **Wrong container or command:** stop the session, change the selection or argv, reconfirm, and start again.
- **Forbidden:** request the least-privilege `pods/exec` permission for the exact namespace; do not retry until access changes.
- **Connection, command, or exit error:** read the terminal message and session status, then inspect Pod status, events, and logs before starting a new session.
- **Scope change:** sessions outside the next workspace or kubeconfig-source scope stop by default. The Live Sessions setting can keep them running across workspace switches; app exit still stops them.
