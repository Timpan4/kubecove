# Engineering Handbook

The handbook is the repo's source of truth for code organization, file size, and hygiene. If another doc contradicts it, fix the other doc or open an issue.

## Pages

- [code-organization.md](code-organization.md) - where files and folders belong.
- [file-size-and-split.md](file-size-and-split.md) - soft/hard caps and how to split oversized files.
- [hygiene.md](hygiene.md) - orphan files, superseded files, duplicates, and dead code.
- [design-system.md](design-system.md) - shared token layer (color, surface, elevation, z-index, typography) for both runtimes.
- [pr-checklist.md](pr-checklist.md) - final checks before opening a PR or marking agent work complete.

## Boundaries

This handbook is not the product vision, architecture blueprint, or ADR log. Use:

- [Product Vision](../product-vision.md) for product direction.
- [Architecture Blueprint](../architecture-blueprint.md) for app shape and Tauri command boundary.
- [decisions/](../decisions/) for locked-in decisions, including guarded cluster operations.

Keep handbook pages short enough to reread during routine work.
