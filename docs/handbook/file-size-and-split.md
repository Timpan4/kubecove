# File Size and Split Triggers

Large files are hard to review and easy to misuse. These caps are the working bar.

## Caps

| Extension | Soft cap | Hard cap |
|-----------|----------|----------|
| `.rs`     | 500 lines | 800 lines |
| `.tsx`    | 400 lines | 700 lines |
| `.ts`     | 300 lines | 600 lines |
| `.css`    | no cap yet | no cap yet |

Exempt:

- `**/*.test.*`
- `**/*.spec.*`
- `**/*.gen.*`
- `**/*.d.ts`
- `bun.lock`
- `node_modules/**`
- `src-tauri/target/**`
- `target/**`

## Meaning

- Soft cap: the hook warns. Plan a split the next time the file is touched for real work.
- Hard cap: the hook fails. Split before committing.

## Legacy Exemptions

Files that already exceeded the hard cap when this policy was introduced live in `.githooks/pre-commit-user` under `LEGACY_OVERSIZED_FILES`.

Rules:

- Structural edits to a legacy oversized file should include a split.
- Remove a file from the legacy list once it drops below the soft cap.
- Do not add new oversized files to the list to silence the hook.

## How to Split

Rust:

- Move command handlers into `commands/<domain>.rs` or `commands/<domain>/`.
- Move shared helpers into `commands/helpers.rs` or a helper submodule.
- Re-export through `commands/mod.rs`.
- Run `bun run rust:check` after each coherent move.

TypeScript and React:

- Move feature subcomponents into sibling files in the same feature folder.
- Move pure feature helpers to `helpers.ts` or a focused sibling module.
- Move truly generic pure logic to `src/lib/`.
- Move reusable generic hooks to `src/hooks/`.
- Run `bun run typecheck` after each coherent move.

Avoid:

- splitting by line count alone
- one-import wrapper files that only dodge the cap
- disabling the hook
- expanding legacy oversized files without a split plan

## Why These Numbers

The caps are designed to catch growth before files become agent-hostile. They can be revisited after the project has enough history to justify different limits.
