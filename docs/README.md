# Docs Index

Read these docs in this order when joining the project or starting a substantial change.

## Product and Architecture

- [Product Vision](product-vision.md) - what KubeCove is, who it serves, and which workflows define the product.
- [Architecture Blueprint](architecture-blueprint.md) - current frontend/backend shape, Tauri command boundary, and extension points.
- [Product Inspiration](product-inspiration.md) - K8Studio and Aptakube as public benchmarks, plus the legal/design boundary.

## Rules

- [Engineering Handbook](handbook/) - file placement, size caps, hygiene rules, and the PR checklist.
- [Agent Guide](../AGENTS.md) - agent-facing rules for this repo, including security boundaries and GitButler workflow.
- [Architecture Decision Records](decisions/) - accepted decisions that require a new ADR to change.

Current ADRs:

- [0001: Local Read-Only Kubernetes API Path](decisions/0001-local-read-only-kube-api.md)
- [0002: Argo CD Native Support Starts Kubernetes-API-First](decisions/0002-argocd-native-kubernetes-api-first.md)

## Work Tracking

- [Milestones](milestones.md) - goal-level project progress and remaining release gates.
- [Task Workflow](task-workflow.md) - Phabricator, Git, MR, and Definition of Done workflow rules.
- [Agent Skill Backlog](agent-skills.md) - project-specific skill ideas to validate before installing.
- [Development Workflow](development-workflow.md) - package manager, hooks, testing, and verification commands.
- [Beta Releases](release.md) - installer guidance, maintainer release flow, smoke tests, and publishing checklist.

## Implementation History

- [superpowers/specs/](superpowers/specs/) - dated design specs.
- [superpowers/plans/](superpowers/plans/) - dated implementation plans.

Those historical docs explain why earlier work was shaped a certain way. Current behavior and rules live in the product, architecture, handbook, ADR, and milestone docs above.
