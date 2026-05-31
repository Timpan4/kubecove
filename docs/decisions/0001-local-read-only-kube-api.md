# ADR 0001: Local Kubernetes API Boundary

## Status

Accepted. Extended by [ADR 0003](0003-guarded-live-sessions.md) for guarded pod port-forwarding and [ADR 0004](0004-guarded-cluster-operations.md) for broader governed cluster operations.

## Context

KubeCove interacts with real Kubernetes clusters, including possible production clusters. The project needs a strict data boundary before adding richer operational workflows.

## Decision

The app is local desktop software. Nothing is deployed into the cluster.

For Kubernetes access:

- Kubernetes credentials stay in local kubeconfig and are handled by the Rust backend.
- React calls typed Tauri commands.
- Normal Kubernetes list/get/discovery/watch operations use `kube-rs`.
- The frontend cannot run arbitrary shell commands.
- Cluster-changing operations are not a generic bridge. They require the governed command model defined in ADR 0004 or a focused ADR such as ADR 0003.

`kubectl`, Helm, and Argo CD may be optional future sidecars or fallbacks. They are not the core data path.

## Consequences

This boundary is slower than shelling out to `kubectl` for every operation, but it makes the app easier to secure, test, and extend. It keeps credential handling, inspection features, and future production-cluster safeguards in one backend-controlled place.
