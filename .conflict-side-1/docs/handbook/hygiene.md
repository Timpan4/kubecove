# Hygiene

Keep the codebase free of dead, duplicate, and misleading code.

## Orphan Files

An orphan file is a source file with no inbound import from production code, excluding tests.

Rules:

- Treat orphan files as bugs.
- Delete an orphan in the same PR that creates or discovers it.
- Do not keep "for reference" copies. Git history is the reference.

Quick local checks:

```sh
rg "from .*/<filename>" src
rg "mod <module>|use .*::<module>" src-tauri/src
```

## Superseded Files

When a PR replaces a component, function, or module:

- Delete the predecessor in the same commit set.
- Do not leave placeholders, commented-out code, or renamed stubs.
- Do not add compatibility re-exports unless an external consumer genuinely depends on the old path.

If the new version is not a drop-in replacement, make that an explicit design conversation.

## Duplicate Components

Avoid two components or helpers that serve the same purpose. Examples:

- two resource detail panels
- two badge systems
- two timestamp formatters
- two selectable-list wrappers

If a duplicate appears during routine work, fold one into the other in a small follow-up PR.

## Dead Code

Delete unused imports, unused exports, unreachable branches, and commented-out blocks.

Rust warns on dead code. Do not add `#[allow(dead_code)]` without a written reason. TypeScript does not catch every unused export, so routine review still matters.

## Allowed

- TODO comments tied to a milestone or ADR follow-up.
- Feature or capability flags that are wired but disabled.
- Explicit examples named `example.*` or kept under a future `playground/` directory.
