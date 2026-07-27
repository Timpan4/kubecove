# Domain Docs

How agent skills should consume this repo's domain documentation when exploring the codebase.

## Before exploring, read these

- `AGENTS.md` for agent-facing implementation rules, safety constraints, and GitButler workflow.
- `docs/product-vision.md` for product direction, workflow model, and safety posture.
- `docs/architecture-blueprint.md` for the frontend/backend shape, Tauri command boundary, and extension points.
- `docs/milestones.md` for current milestone status and completion tracking.
- `docs/development-workflow.md` for workflow conventions and verification commands.
- Relevant ADRs in `docs/decisions/` before changing Kubernetes access, Tauri command boundaries, guarded operations, Argo CD behavior, or other security-sensitive paths.
- `CONTEXT.md` at the repo root if it exists.

If `CONTEXT.md` does not exist, proceed silently. Do not flag its absence or suggest creating it upfront. The producer skill (`/grill-with-docs`) creates it lazily when terms or decisions actually get resolved.

## File structure

This is a single-context repo:

```text
/
|-- AGENTS.md
|-- docs/
|   |-- product-vision.md
|   |-- architecture-blueprint.md
|   |-- milestones.md
|   |-- development-workflow.md
|   `-- decisions/
`-- src/
```

## Use project vocabulary

When your output names a domain concept, use the terms from `AGENTS.md`, the product docs, architecture docs, ADRs, and `CONTEXT.md` if present. Do not drift to synonyms where the docs already define a term.

If the concept you need is not documented yet, either reconsider the terminology or note the gap for `/grill-with-docs`.

## Flag ADR conflicts

If your output contradicts an existing ADR, surface it explicitly instead of silently overriding it:

> Contradicts ADR 0004 (Guarded Cluster Operations), but worth reopening because...
