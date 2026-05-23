# ADR 0001: Local Read-Only Kubernetes API Path

## Status

Accepted.

## Context

KubeCove interacts with real Kubernetes clusters, including possible production clusters. The project needs a strict data boundary before adding richer operational workflows.

## Decision

The app is local desktop software. Nothing is deployed into the cluster.

For the default product path:

- Kubernetes credentials stay in local kubeconfig and are handled by the Rust backend.
- React calls typed Tauri commands.
- Normal Kubernetes list/get/discovery/watch operations use `kube-rs`.
- The frontend cannot run arbitrary shell commands.
- Cluster mutations are out of scope until a future ADR defines the workflow and guardrails.

`kubectl`, Helm, and Argo CD may be optional future sidecars or fallbacks. They are not the core data path.

## Consequences

This boundary is slower than shelling out to `kubectl` for every operation, but it makes the app easier to secure, test, and extend. It also keeps credential handling and future production-cluster safeguards in one backend-controlled place.
