# ADR 0001: Local Read-Only Kubernetes API Path

## Status

Accepted.

## Context

The app will eventually interact with real Kubernetes clusters, possibly production clusters. The MVP needs to establish a safe data boundary before adding richer workflows.

## Decision

For the MVP:

- The app is local desktop software.
- Nothing is deployed into the cluster.
- Kubernetes credentials stay in local kubeconfig and are handled by the Rust backend.
- React calls typed Tauri commands.
- The Rust backend uses `kube-rs` for normal Kubernetes list/get/discovery operations.
- The first milestone is read-only.
- The frontend cannot run arbitrary shell commands.

`kubectl`, Helm, and Argo CD may be added later as optional sidecars or fallbacks, but they are not the core data path.

## Consequences

This creates a stricter implementation boundary early. It may take slightly longer to build the MVP than shelling out to `kubectl`, but the app will be easier to secure, test, and extend.
