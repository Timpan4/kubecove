# ADR 0002: Argo CD Native Support Starts Kubernetes-API-First

## Status

Accepted.

## Context

Argo CD is a core workflow for many Kubernetes users. The app should understand Argo-managed resources and applications natively, not only as a future side integration.

At the same time, the MVP constraints still apply:

- local desktop app
- no cluster-side deployment
- read-only first milestone
- no arbitrary frontend shell execution
- Kubernetes access through Rust-side Tauri commands

## Decision

Argo CD support starts by reading Argo CD Kubernetes resources and tracking metadata through `kube-rs`.

Initial support should:

- detect Argo-managed resources from labels and annotations
- list `Application`, `ApplicationSet`, and `AppProject` resources when their CRDs exist
- summarize Application sync and health status from resource fields
- group related Kubernetes resources by Argo application when tracking metadata is available
- expose read-only Argo details through typed Tauri commands

The Argo CD API, Argo CD CLI, sync, rollback, diff, and mutation workflows are future features. They require separate ADRs and explicit permission/UX guardrails.

## Consequences

This keeps Argo CD support aligned with the app's Kubernetes-native architecture and avoids introducing a second authentication surface too early. Some Argo CD features will be unavailable until API support is intentionally designed.
