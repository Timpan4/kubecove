# Hygiene

Rules for keeping the codebase free of dead, duplicate, and orphaned code.

## Orphan files

An **orphan file** is a source file with no inbound import from production code (excluding tests).

- An orphan file is a bug. It misleads anyone reading the code.
- If you find one, delete it in the same PR you noticed it in.
- Don't leave "for reference" copies. Git history is the reference.

How to check (quick local sweep):

```sh
# TypeScript / TSX
grep -rln "from .*/<filename>" src/ || echo "<filename> looks orphaned"

# Rust
grep -rln "mod <module>\|use .*::<module>" src-tauri/src/
```

## Superseding a file

When a PR replaces a component, function, or module with a new one:

- The PR deletes the predecessor in the same commit set.
- No `// removed` placeholders. No renamed-and-kept stubs. No re-exports for backwards compatibility unless an external consumer genuinely depends on the old path.
- If the new version isn't a drop-in replacement, that's its own design conversation — don't merge two halves of a transition and call it done.

## Duplicate components

If two components serve the same purpose, the PR that introduces the second one is wrong. Examples to avoid:

- Two detail panels for the same resource inspector.
- Two badge components, two timestamp formatters, two "selectable list" wrappers.

When you notice a duplicate during routine work, fold one into the other in a short follow-up PR. Don't leave both.

## Dead code inside a file

- Unused imports, unused exports, unreachable branches, commented-out blocks. Delete them.
- The Rust compiler warns on dead code (`#[warn(dead_code)]`). Don't suppress it with `#[allow(dead_code)]` unless there's a written reason.
- TypeScript: `bun run typecheck` won't catch every unused export. A periodic eyeballing pass is fine; a `knip` integration is a future option.

## What's deliberately allowed

- TODO comments tied to a milestone or ADR follow-up. They state intent and unblock current work.
- Feature flags or capability flags that are wired through but currently disabled — these are not dead code; they're switches.
- One-screen example/demo files explicitly named `example.*` or living under a `playground/` directory. (Currently none exist; the rule is preemptive.)
