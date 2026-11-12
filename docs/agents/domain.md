# Domain Docs

What agent skills read before exploring code.

## Read First

- `AGENTS.md` for implementation rules and safety constraints.
- `docs/product-and-architecture.md` for consolidated product direction and app shape.
- `docs/development-workflow.md` for commands and verification.
- `docs/agents/issue-tracker.md` for GitHub Issues tracking.
- Relevant `docs/decisions/` before Kubernetes access, Tauri boundaries, guarded operations, Argo CD, or security-sensitive paths.
- `CONTEXT.md` at the repository root for terms.

`CONTEXT.md` defines the project vocabulary.

## Tracking

Use GitHub Issues for planned work, follow-ups, and issue status. Use GitHub Milestones for delivery grouping. Repository Markdown checklists are not the issue tracker.

## File Structure

This is a single-context repository:

```text
/
|-- AGENTS.md
|-- CONTEXT.md
|-- docs/
|   |-- product-and-architecture.md
|   |-- development-workflow.md
|   |-- agents/issue-tracker.md
|   `-- decisions/
`-- src/
```

## Vocabulary and ADRs

Use the terms defined by `CONTEXT.md`, `AGENTS.md`, the product and architecture document, and the ADRs. If a concept is undocumented, reconsider the term or record the gap in a GitHub Issue.

Surface ADR conflicts instead of silently overriding them:

> Contradicts ADR 0004 (Guarded Cluster Operations), but worth reopening because...
