# `src/components/`

Generic, feature-agnostic UI belongs here.

Allowed:

- shadcn primitives under `ui/svelte/`
- shared layout chrome
- reusable display helpers such as `TimestampText`, `CopyableText`, `StatGrid`, and `SimpleTable`
- generic skeletons, empty states, badges, tooltips, and form controls

Not allowed:

- components used by exactly one feature
- components that import from `src/features/`
- feature-specific wrappers kept here for convenience
- operation-specific confirmation flows that belong to a product feature

The older flat component `ClusterSelector` still lives here. Move it into a feature folder when touching its structure. New feature-specific components should start in `src/features/<area>/`.

See [docs/handbook/code-organization.md](../../docs/handbook/code-organization.md).
