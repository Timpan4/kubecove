# `src/features/`

One folder per product area. Each feature is **self-contained**: components, hooks, helpers, types specific to the area live inside the folder. Other features import only the feature's public surface — named exports from a top-level entry — not deep paths into private helpers.

Current areas: `argo/`, `resource-detail/`, `resources/`, `settings/`, `workspaces/`.

A new feature folder is created when a product area has more than one component or hook of its own and that code isn't reusable outside the area. Generic primitives still go to `src/components/`. Pure logic shared across features goes to `src/lib/`. See [docs/handbook/code-organization.md](../../docs/handbook/code-organization.md).

Caps: `.tsx` soft 400, hard 700; `.ts` soft 300, hard 600. When a feature panel grows large, pull subcomponents into sibling files and pure logic into a `helpers.ts` inside the folder. See [docs/handbook/file-size-and-split.md](../../docs/handbook/file-size-and-split.md).
