# Domain Docs

How agent skills consume domain docs before code exploration.

## Read First

- `AGENTS.md` for implementation rules and safety constraints.
- `docs/product-and-architecture.md` for consolidated product direction and app shape.
- `docs/development-workflow.md` for commands and verification.
- `docs/agents/issue-tracker.md` for GitHub Issues tracking.
- Relevant `docs/decisions/` before Kubernetes access, Tauri boundaries, guarded operations, Argo CD, or security-sensitive paths.
- `CONTEXT.md` at repo root when present.

If `CONTEXT.md` is absent, proceed silently. Do not suggest creating it before unresolved terms or decisions need it.

## Tracking

Use GitHub Issues for planned work, follow-ups, and issue status. Use GitHub Milestones for delivery grouping. Do not treat repository markdown checklists as issue tracker.

## File Structure

This is single-context repo:

```text
/
|-- AGENTS.md
|-- docs/
|   |-- product-and-architecture.md
|   |-- development-workflow.md
|   |-- agents/issue-tracker.md
|   `-- decisions/
`-- src/
```

## Vocabulary and ADRs

Use terms defined by `AGENTS.md`, product and architecture docs, ADRs, and `CONTEXT.md` when present. If concept is undocumented, reconsider term or record gap in GitHub Issue.

Surface ADR conflicts instead of silently overriding them:

> Contradicts ADR 0004 (Guarded Cluster Operations), but worth reopening because...
