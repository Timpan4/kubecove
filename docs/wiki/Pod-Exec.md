# Pod Exec

Start an interactive process only from the detail page of one namespaced Pod.

Related: [Resource Details](https://github.com/Timpan4/kubecove/wiki/Resource-Details) · [Port Forwarding](https://github.com/Timpan4/kubecove/wiki/Port-Forwarding) · [RBAC and Permissions](https://github.com/Timpan4/kubecove/wiki/RBAC-and-Permissions)

## Prerequisites

- Select the exact Pod in a namespace, then open **Exec** in Resource Details.
- Use Kubernetes access authorized for `pods/exec` in that namespace.
- Treat the session as direct container access. Check the context, namespace, Pod, container, and command before you open a session.

Exec does not start from a Service, Deployment, Job, owner, or selector.

## Open a session

With no session open, the tab shows **No exec sessions** and two ways to start one:

- **Open /bin/sh in** the default container starts exactly `['/bin/sh']` there.
- **Other** lists `/bin/sh` and `/bin/bash` for each container in the Pod, and **Custom command…**.

The button or menu item you select names the container and command, and selecting it is the confirmation. There is no separate checkbox or start step.

**Custom command…** takes a container and **Arguments, one per line**. Arguments are passed as written, without local shell parsing. For example:

```text
/usr/bin/env
printenv
```

This sends `['/usr/bin/env', 'printenv']`; it does not run through a local shell. Use an explicit shell and arguments when the container requires shell syntax.

## Use the terminal

Each session opens in its own tab. **New exec session** (the **+** button) opens another one beside it, so several sessions can run in the same Pod.

The terminal streams process output and sends keyboard input to Kubernetes. Resizing the terminal sends the new dimensions to the session. The toolbar has these controls:

- **Stop session** ends the process and keeps the tab.
- **Restart session** starts the same command in the same container again.
- **Clear terminal** clears the displayed output only. It does not alter the process or create a record.
- **Stop and close** on a tab ends its session and removes the tab.

Sessions stop when you close the Pod or switch to another resource. Active sessions are also listed under **Pod exec sessions** in the Live Sessions view, where you can stop them.

Output is kept only in the terminal view. There is no transcript, and sessions are not restored after an app restart.

## Verify and recover

- **Shell not found:** select the shell the image provides, or use **Custom command…** with an executable path.
- **Wrong container or command:** stop and close the session, then open a new one with the right container and command.
- **Forbidden:** request the least-privilege `pods/exec` permission for the exact namespace; do not retry until access changes.
- **Connection, command, or exit error:** read the terminal message and session status, then inspect Pod status, events, and logs before starting a new session.
- **Scope change:** sessions outside the next workspace or kubeconfig-source scope stop by default. The Live Sessions setting can keep them running across workspace switches; app exit still stops them.
