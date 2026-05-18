# Docs Index

Documentation for the KubeCove desktop Kubernetes workspace. Read in roughly this order — vision first, then the constraints that fall out of it, then the work tracker.

## Start here

- [Product Vision](product-vision.md) — what this app is, who it's for, the navigation model, GitOps posture, and safety posture.
- [Architecture Blueprint](architecture-blueprint.md) — backend/frontend module layout, Tauri command contracts, MVP resource strategy, future extension points.

## Conventions and handbook

- [Engineering Handbook](handbook/) — file-size caps, module boundaries, hygiene rules, PR checklist. The source of truth for "where does this go" and "is this file too big." Module folders under `src/` and `src-tauri/src/` also have a short `README.md` next to the code.

## Constraints and decisions

- [Architecture Decision Records](decisions/) — locked-in decisions that require an ADR to change. Currently: [0001 local read-only Kube API](decisions/0001-local-read-only-kube-api.md), [0002 Argo CD native Kubernetes-API-first](decisions/0002-argocd-native-kubernetes-api-first.md).
- [Agent Guide](../AGENTS.md) — agent-facing rules: security boundary, command contracts, when an ADR is required, verification expectations.

## Work tracking

- [Milestones](milestones.md) — single source of truth for goal-level progress. Includes cross-cutting tracks for security and agent skills.
- [Agent Skill Backlog](agent-skills.md) — project-specific skill ideas to write before installing as real skills.
- [Development Workflow](development-workflow.md) — Bun/cargo commands, pre-commit hook, verification expectations.

## Context

- [Product Inspiration](product-inspiration.md) — K8Studio and Aptakube reference, legal/design boundary for not copying their work.

## Implementation history

- [superpowers/plans/](superpowers/plans/) — dated implementation plans for completed or in-progress milestones.
- [superpowers/specs/](superpowers/specs/) — dated design specs that those plans implement.
