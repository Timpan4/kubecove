# ADR 0004: Guarded Cluster Operations

## Status

Accepted.

## Context

KubeCove started as an inspection-first Kubernetes workspace and has begun adding governed live operations, starting with pod port-forwarding in [ADR 0003](0003-guarded-live-sessions.md). The broader product direction should allow cluster-changing workflows when they are intentionally designed and safe enough for real operator use.

The existing boundary still matters: KubeCove is local desktop software, kubeconfig material stays Rust-side, React is not trusted with credentials, and the frontend must not become a shell or Kubernetes bridge.

## Decision

KubeCove may add cluster-changing operations only through governed product workflows.

Every cluster-changing workflow must have:

- a narrow Rust-side Tauri command, not a generic command runner
- typed Rust serde request and response models
- a TypeScript mirror type and typed frontend wrapper
- visible target context, namespace, resource kind, and resource name before execution
- an explicit confirmation step for destructive or broad operations
- user-visible errors that preserve enough detail to recover safely
- permission-aware UX that makes Kubernetes authorization failures understandable
- dry-run, diff, preview, or staged confirmation when the Kubernetes API supports it

The frontend still cannot run arbitrary shell commands, read raw kubeconfig contents, or receive tokens, certificates, or broad filesystem data.

Normal Kubernetes API access should continue to use `kube-rs`. CLI-backed behavior, Argo CD API flows, Helm actions, sync, rollback, diff, exec, terminals, or broad local filesystem access need a focused ADR when they add a distinct authentication surface, shell surface, or mutation model. Pod port-forwarding is governed separately by ADR 0003.

Docs and release notes must distinguish shipped behavior from planned guarded operations. Do not claim apply, delete, scale, sync, rollback, or exec support until the corresponding typed command and UX path exist.

## Consequences

KubeCove is no longer described as a permanently read-only product. It is inspection-first today and mutation-ready through governed operations.

This adds design overhead before mutating features, but it keeps production-cluster safety, credential handling, and reviewability aligned with the existing Tauri boundary.
