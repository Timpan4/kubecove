# `src/lib/`

Pure logic. **No JSX.** No feature-specific behavior.

Belongs here:
- Typed wrappers around Tauri commands (`tauri.ts`).
- Frontend mirrors of serde contracts (`types.ts`).
- The Zustand store and composed hooks (`hooks.ts`).
- Settings persistence (`settings.ts`).
- Pure helpers shared across features (`tree-nav.ts`, `resource-visuals.ts`, `diagnostics.ts`, `utils.ts`).

Does **not** belong here:
- React components. Those go in `src/components/` (generic) or `src/features/<area>/` (specific).
- Anything that imports from `src/features/` or `src/components/`. `lib/` is upstream of both.
- Logic that only one feature uses — move it into that feature.

Caps: `.ts` soft 300 lines, hard 600. See [docs/handbook/file-size-and-split.md](../../docs/handbook/file-size-and-split.md).
