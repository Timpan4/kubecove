# `src/components/`

Generic, feature-agnostic UI belongs here.

Allowed:

- shadcn primitives under `ui/`
- shared layout chrome
- reusable display helpers such as `MetadataBadges`, `StatusBadge`, and `TimestampText`
- generic skeletons, empty states, badges, tooltips, and form controls

Not allowed:

- components used by exactly one feature
- components that import from `src/features/`
- feature-specific wrappers kept here for convenience

Older flat components (`ClusterSelector`, `NamespaceList`, `KindList`, `SidebarTree`) still live here. Move them into feature folders when touching their structure. New feature-specific components should start in `src/features/<area>/`.

See [docs/handbook/code-organization.md](../../docs/handbook/code-organization.md).
