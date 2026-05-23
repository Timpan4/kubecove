# `src/lib/`

Pure shared logic belongs here. No JSX.

Allowed:

- typed Tauri wrappers in `tauri.ts`
- frontend mirrors of serde contracts in `types.ts`
- Zustand stores and composed dashboard hooks in `hooks.ts`
- workspace, settings, query-key, release-channel, diagnostics, and formatting helpers
- pure resource health, metrics, visuals, and tree-navigation logic

Not allowed:

- React components
- imports from `src/features/`
- imports from `src/components/`
- logic that only one feature uses

Feature-only logic belongs beside that feature. Generic React hooks belong in `src/hooks/`.

Caps: `.ts` soft 300 / hard 600. See [docs/handbook/file-size-and-split.md](../../docs/handbook/file-size-and-split.md).
