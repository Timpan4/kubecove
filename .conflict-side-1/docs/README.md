# KubeCove Docs

Use this index as the map for current product, engineering, release, and governance work. Latest published release is `0.9.1`; product and architecture docs describe current source and label post-release behavior as unreleased.

## Users and Testers

- [README](../README.md) - product overview, installer links, development quick start, and safety model.
- [Beta Releases](release.md) - installer guidance, updater behavior, smoke tests, and publishing checks.
- [Milestones](milestones.md) - current beta baseline, release gates, and tracked follow-up work.

## Contributors

- [Development Workflow](development-workflow.md) - package manager, hooks, tests, and verification commands.
- [Engineering Handbook](handbook/) - file placement, size caps, hygiene rules, and the PR checklist.
- [Agent Guide](../AGENTS.md) - agent-facing implementation rules, GitButler workflow, and security boundaries.
- [Agent Skill Backlog](agent-skills.md) - project-specific skill ideas to validate before installing.

## Product and Architecture

- [Product Vision](product-vision.md) - audience, workflow model, product direction, and safety posture.
- [Architecture Blueprint](architecture-blueprint.md) - frontend/backend shape, Tauri command boundary, and extension points.
- [Product Inspiration](product-inspiration.md) - public product benchmarks and the design/legal boundary.
- [Helm Reconciliation Design](helm-reconciliation-design.md) - inspection-only release intent versus live state comparison.
- [Deterministic E2E Lab](development-workflow.md#deterministic-e2e-and-kind-lab) - fast mocks, isolated real Kind runs, and the reusable development cluster.

## Governance

- [ADR 0001: Local Kubernetes API Boundary](decisions/0001-local-read-only-kube-api.md)
- [ADR 0002: Argo CD Native Support Starts Kubernetes-API-First](decisions/0002-argocd-native-kubernetes-api-first.md)
- [ADR 0003: Guarded Live Kubernetes Sessions](decisions/0003-guarded-live-sessions.md)
- [ADR 0004: Guarded Cluster Operations](decisions/0004-guarded-cluster-operations.md)
- [ADR 0005: Guarded Pod Exec Sessions](decisions/0005-guarded-pod-exec-sessions.md)
- [ADR 0006: Guarded Selected-Resource YAML Apply](decisions/0006-guarded-selected-resource-yaml-apply.md)
- [ADR 0007: GitOps Providers Are Kubernetes-API-First And Inspection-Only By Default](decisions/0007-gitops-providers-kubernetes-api-first.md)
- [ADR 0008: Parallel Svelte Frontend Migration](decisions/0008-svelte-frontend-migration.md)
- [ADR 0009: Workspace-Scoped Kubernetes Request Cancellation](decisions/0009-workspace-request-cancellation.md)
- [ADR 0010: E2E-Only WDIO Security Boundary](decisions/0010-e2e-only-wdio-security-boundary.md)
- [ADR 0011: Rolling Three-Minor Kubernetes Support](decisions/0011-rolling-kubernetes-support.md)
- [ADR 0012: Production-Shaped E2E Lab](decisions/0012-production-shaped-e2e-lab.md)
- [ADR 0013: Argo CD Connected Inspection And Operations](decisions/0013-argocd-connected-inspection-and-operations.md)
- [ADR 0014: Runtime Secret Disclosure](decisions/0014-runtime-secret-disclosure.md)
- [ADR 0015: Flux Inspection Roadmap](decisions/0015-flux-inspection-roadmap.md)

New work that changes the frontend/backend security boundary, Kubernetes access path, Tauri permissions, cluster-changing behavior, or GitOps provider API/CLI integration beyond accepted ADRs needs a focused decision before implementation.
