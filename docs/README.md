# KubeCove Docs

Use this index as the map for product, engineering, release, and governance work. Historical specs stay under `superpowers/`; current rules live in the product, architecture, handbook, ADR, and release docs below.

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
- [Svelte Frontend Migration Plan](svelte-frontend-migration-plan.md) - cutover status, cleanup notes, and verification.
- [Product Inspiration](product-inspiration.md) - public product benchmarks and the design/legal boundary.
- [Helm Reconciliation Design](helm-reconciliation-design.md) - inspection-only release intent versus live state comparison.

## Governance

- [ADR 0001: Local Kubernetes API Boundary](decisions/0001-local-read-only-kube-api.md)
- [ADR 0002: Argo CD Native Support Starts Kubernetes-API-First](decisions/0002-argocd-native-kubernetes-api-first.md)
- [ADR 0003: Guarded Live Kubernetes Sessions](decisions/0003-guarded-live-sessions.md)
- [ADR 0004: Guarded Cluster Operations](decisions/0004-guarded-cluster-operations.md)
- [ADR 0005: Guarded Pod Exec Sessions](decisions/0005-guarded-pod-exec-sessions.md)
- [ADR 0007: GitOps Providers Are Kubernetes-API-First And Inspection-Only By Default](decisions/0007-gitops-providers-kubernetes-api-first.md)
- [ADR 0008: Parallel Svelte Frontend Migration](decisions/0008-svelte-frontend-migration.md)

New work that changes the frontend/backend security boundary, Kubernetes access path, Tauri permissions, cluster-changing behavior, or GitOps provider API/CLI integration needs an ADR before implementation.

## Historical Specs

- [superpowers/specs/](superpowers/specs/) - dated design specs.
- [superpowers/plans/](superpowers/plans/) - dated implementation plans.

These files explain earlier decisions and should not be treated as the current contract when they differ from the docs above.
