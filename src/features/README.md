# `src/features/`

Each folder is one product area. Feature-specific components, hooks, helpers, and types live together in that folder.

Current areas:

- `app-updates/`
- `argo/`
- `command-palette/`
- `helm/`
- `live-sessions/`
- `rbac/`
- `resource-detail/`
- `resources/`
- `settings/`
- `workspaces/`

Import across features only through a public entry point when one exists. Do not deep-import private helpers from another feature.

Future guarded operation UI belongs in the product feature it serves and must keep context, namespace, kind, and resource target visible before execution.

Use:

- `src/components/` for generic UI
- `src/lib/` for shared pure logic
- `src/hooks/` for generic React hooks

Caps: `.tsx` soft 400 / hard 700, `.ts` soft 300 / hard 600. Split large feature files into sibling components, hooks, or helpers with coherent responsibilities.

See [docs/handbook/code-organization.md](../../docs/handbook/code-organization.md) and [docs/handbook/file-size-and-split.md](../../docs/handbook/file-size-and-split.md).
