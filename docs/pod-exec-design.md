# Guarded Pod Exec Design

## Goal

KubeCove 0.5.0 adds a narrow Pod exec workflow for troubleshooting a selected container without adding a generic shell runner or broad mutation surface.

## V1 Behavior

- Entry point: Pod detail panel only.
- Target: selected cluster context, namespace, Pod name, and optional container.
- Commands: `/bin/sh`, `/bin/bash`, or custom argv entered one item per line.
- Confirmation: user must acknowledge the exact target and command before start.
- Runtime: Rust opens `kube-rs` `pods/exec`, streams terminal output over a typed Tauri channel, accepts stdin and resize through typed commands, and stores only in-memory session summaries.
- Lifecycle: sessions can be stopped by the user, are aborted on app shutdown, and are not restored or saved.

## Safety Contract

- No kubeconfig, token, certificate, or raw credential material reaches React.
- React cannot execute local shell commands.
- The backend exposes only Pod-scoped exec commands, not a generic Kubernetes operation API.
- RBAC failures, missing shells, disconnected sessions, and validation errors are shown as user-facing errors.
- Output is not written to durable workspace state.

## Deferred Work

- Service, Deployment, Job, and owner-backed exec.
- Saved exec commands or auto-start behavior.
- YAML apply, delete, scale, restart, Argo CD sync/rollback/diff, and Helm actions.
- Durable terminal transcript history.
