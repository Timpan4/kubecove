# ADR 0015: Flux Inspection Roadmap

## Status

Accepted.

## Decision

Flux remains Kubernetes-API-first and inspection-only. Roadmap: (1) normalize existing CRD status and inventory inspection, (2) add typed compare/read surfaces when Flux APIs expose them, (3) design guarded reconcile/suspend operations in a separate ADR. No Flux runtime command, CLI integration, mutation, dependency, fixture, or UI is introduced by this Argo CD parity change.
