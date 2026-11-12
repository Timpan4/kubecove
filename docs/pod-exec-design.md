# Guarded Pod Exec Design

## Scope

KubeCove provides exact-Pod exec for troubleshooting selected namespaced Pods. It is not a generic shell runner or a broad Kubernetes-operation API.

## Current Lifecycle

The Svelte `ExecTab` is available only from the detail of a selected Pod. It builds each request from the selected context, namespace, Pod name, container, terminal size, and argv. The user opens `/bin/sh` or `/bin/bash` in a named container, or enters a custom argv with one item per line. The labeled open or restart control is the confirmation; the UI has no separate acknowledgement step.

Each session has its own tab, terminal and typed channel, and several sessions can run in one Pod. Terminal input and resize send typed commands for that tab's session only. Status, output, errors, exit, and stop messages update the terminal and the session query. Closing a tab, closing the Pod detail or switching resource stops the affected sessions and disposes their terminals.

Rust validates a non-empty context, namespace, Pod, and command; terminal dimensions; TTY and stdin compatibility; the acknowledgement field of the request; and that the confirmation's target and command equal the request's. It opens `kube-rs` `pods/exec`, streams through the typed channel, and accepts only typed stdin, resize, and stop requests.

## Narrow Safety Boundary

- The frontend cannot execute local shell commands or generic Kubernetes operations.
- The backend exposes Pod-scoped exec commands only.
- Kubeconfig, tokens, certificates, and raw credentials never reach the frontend.
- RBAC failures, missing shells, disconnects, and validation errors are shown to the user.
- Output and session summaries are in memory only. Sessions stop on app shutdown and are never restored.

See [ADR 0005](decisions/0005-guarded-pod-exec-sessions.md).

## Deferred

Outside the current Pod exec scope:

- Service, Deployment, Job, or owner-backed target resolution.
- Saved commands, auto-start, or durable transcripts.
- YAML apply, other guarded resource operations, connected Argo CD operations, and Helm actions.

Deferred workflows require their own narrow contracts. They do not expand the Pod exec boundary.
