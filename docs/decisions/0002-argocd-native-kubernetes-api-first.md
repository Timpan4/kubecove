# ADR 0002: Argo CD Native Support Starts Kubernetes-API-First

## Status

Accepted. Extended by [ADR 0013](0013-argocd-connected-inspection-and-operations.md) for explicit connected HTTP API inspection and guarded operations.

## Context

Argo CD is a common Kubernetes workflow, and KubeCove should understand Argo-managed applications natively. The core safety constraints still apply: local desktop app, no cluster-side deployment, no arbitrary frontend shell execution, and Kubernetes access through Rust-side Tauri commands.

## Decision

Argo CD support starts by reading Argo CD Kubernetes resources and tracking metadata through `kube-rs`.

Initial support:

- detect Argo-managed resources from labels and annotations
- list `Application`, `ApplicationSet`, and `AppProject` resources when CRDs exist
- summarize Application sync and health status from resource fields
- group related Kubernetes resources by Argo application when tracking metadata exists
- expose Argo inspection details through typed Tauri commands

The Argo CD API, Argo CD CLI, sync, rollback, diff, and mutation workflows are future features. Each requires a focused ADR or an explicit ADR 0004-compatible design with permission-aware UX guardrails.

## Consequences

This keeps Argo support aligned with the Kubernetes-native architecture and avoids adding a second authentication surface too early. Some Argo CD capabilities remain unavailable until API support is intentionally designed.
