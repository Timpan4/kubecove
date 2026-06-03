# ADR 0005: Guarded Pod Exec Sessions

## Status

Accepted.

## Context

KubeCove now supports guarded port-forward live sessions under [ADR 0003](0003-guarded-live-sessions.md). Pod exec is a stronger operational capability because it opens an interactive process inside a running container. It must not turn the React frontend into a shell bridge, expose kubeconfig material, or create a broad Kubernetes operation runner.

## Decision

Pod exec v1 is limited to exact namespaced Pods.

- Exec starts only from a selected Pod detail surface.
- The target context, namespace, Pod, optional container, and argv are shown before start.
- Interactive shell presets are exact `/bin/sh` and `/bin/bash` commands; there is no silent fallback.
- Custom commands are passed as explicit argv, not parsed by a local shell.
- Each start requires explicit user confirmation.
- Sessions are in-memory only, visible in the Pod detail surface, and stoppable.
- Sessions are not saved, restored, or auto-started across app restarts.
- Output is streamed to the active UI session and is not persisted as local history.
- Kubernetes RBAC remains authoritative for `pods/exec`; forbidden and command errors are surfaced as user-visible errors.
- Terminal resize and stdin travel through narrow typed Tauri commands.
- The frontend never receives kubeconfig contents, tokens, certificates, broad filesystem access, or local shell execution capability.

Service, Deployment, Job, owner-backed, Argo CD, Helm, YAML apply, delete, scale, restart, sync, rollback, and diff operations remain out of scope for this ADR.

## Consequences

KubeCove can support a useful operational shell path without becoming a generic terminal. The design adds a second live-session registry next to port-forwarding, but keeps Kubernetes access Rust-side and keeps the session target explicit.
