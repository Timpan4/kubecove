# File Size and Split Triggers

A file that's too long is unreadable, agent-hostile, and hides duplication. These caps are the bar.

## Caps

| Extension | Soft (warn)  | Hard (fail)  |
|-----------|--------------|--------------|
| `.rs`     | 500 lines    | 800 lines    |
| `.tsx`    | 400 lines    | 700 lines    |
| `.ts`     | 300 lines    | 600 lines    |
| `.css`    | no cap yet   | no cap yet   |

Tests, generated files, and lockfiles are exempt: `**/*.test.*`, `**/*.spec.*`, `**/*.gen.*`, `**/*.d.ts`, `bun.lock`, `node_modules/**`, `src-tauri/target/**`, `target/**`.

## What the caps mean

- **Soft cap** — the pre-commit hook warns. Plan a split soon. Treat as a checklist item in the next PR that touches the file.
- **Hard cap** — the pre-commit hook fails. Split before committing.

## Legacy exemptions

Files that already exceed the hard cap when this policy was introduced are listed in `.githooks/pre-commit-user` under `LEGACY_OVERSIZED_FILES`. They are allowed to be committed as-is, but:

- Any structural edit to a legacy file should be paired with a split, not new growth.
- The list is drained over time. A file is removed from the list when it drops below the soft cap.
- The list is not a place to silence new offenders. New oversized files are not added.

## How to split

**Rust (`src-tauri/src/commands.rs`-style growth):**
- Move related commands into a per-domain file (`commands/<domain>.rs`).
- Move shared helpers into `commands/helpers.rs`.
- Re-export from `commands/mod.rs` so `lib.rs`'s `invoke_handler!` registration doesn't need a sweeping rewrite — just update the import path.
- `cargo check --manifest-path src-tauri/Cargo.toml` after each move.

**TypeScript / React (`*.tsx` growth):**
- Pull subcomponents out into sibling files inside the same feature folder.
- Pull pure functions (formatting, mapping, derived state) into a `feature-helpers.ts` or into `src/lib/` if truly generic.
- Pull custom hooks into a `hooks.ts` next to the component.
- `bun run typecheck` after each move.

**Don't:**
- Don't split by line count alone. Each split should produce files with a coherent single responsibility.
- Don't introduce a one-import wrapper just to dodge the cap.
- Don't disable the hook. If a hard cap genuinely needs an exception, add the file to the legacy list with a code comment explaining why and a follow-up issue.

## Why these numbers

`src-tauri/src/commands.rs` hit 2525 lines before anyone flagged it. `src/components/ResourceList.tsx` hit 993. The caps are calibrated so the next file to grow past them is caught well before reaching that state. Soft caps can be re-tuned after the project has lived with them for a month.
