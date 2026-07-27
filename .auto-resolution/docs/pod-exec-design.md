# Guarded Pod Exec Design

## Scope

KubeCove provides exact-Pod exec for troubleshooting selected namespaced Pods. It is not generic shell runner or broad Kubernetes-operation API.

## Current Lifecycle

Svelte `ExecTab` is available only from selected Pod detail. It builds request from selected context, namespace, Pod name, optional container, terminal size, and argv. User chooses `/bin/sh`, `/bin/bash`, or custom argv entered one item per line, then acknowledges exact target and serialized command.

Svelte creates terminal and typed Tauri channel on mount; terminal input and resize send only active session's typed commands. Starting a new session closes channel and stops prior active session. Status, output, errors, exit, and stop messages update terminal and session query. Unmount closes channel, stops active session, and disposes terminal. Sessions are listed by polling and filtered to selected exact resource.

Rust validates non-empty context, namespace, Pod, and command; terminal dimensions; TTY/stdin compatibility; acknowledgement; and confirmation target and command equality. It opens `kube-rs` `pods/exec`, streams through typed channel, and accepts only typed stdin, resize, and stop requests.

## Narrow Safety Boundary

- Frontend cannot execute local shell commands or generic Kubernetes operations.
- Backend exposes Pod-scoped exec commands only.
- Kubeconfig, tokens, certificates, and raw credentials never reach frontend.
- RBAC failures, missing shells, disconnects, and validation errors are user-facing.
- Output and session summaries are in memory only; sessions stop on app shutdown and are never restored.

See [ADR 0005](decisions/0005-guarded-pod-exec-sessions.md).

## Deferred

Not current Pod exec scope:

- Service, Deployment, Job, or owner-backed target resolution.
- Saved commands, auto-start, or durable transcripts.
- YAML apply, other guarded resource operations, connected Argo CD operations, and Helm actions.

Deferred workflows require their own narrow contracts; they do not expand Pod exec boundary.
