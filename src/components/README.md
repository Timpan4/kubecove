# `src/components/`

Only **generic, feature-agnostic** reusables.

Belongs here:
- `ui/` — shadcn primitives (button, input, select, tabs, tooltip, badge).
- Layout shells and chrome shared across features.
- Generic display helpers like `MetadataBadges`, `TimestampText`.

Does **not** belong here:
- A component used by exactly one feature. Move it into `src/features/<area>/`.
- Anything that imports from a specific feature folder. That's a sign it's not generic.

See [docs/handbook/code-organization.md](../../docs/handbook/code-organization.md).

Note: at the time this handbook landed, several feature-specific components still live here flat (`ClusterSelector`, `NamespaceList`, `KindList`, `SidebarTree`). They're slated to move into feature folders in a follow-up pass — new components should not be added here unless they're truly generic.
