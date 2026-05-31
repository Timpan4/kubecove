# Development Workflow

## Package Manager

Use Bun for frontend dependencies, scripts, and scaffolding.

Common commands:

```sh
bun install
bun run tauri dev
bun run typecheck
bun test
bun run rust:test
bun run rust:check
bun run check
```

`bun run check` runs the current local verification bundle:

```sh
bun run typecheck && bun test && bun run rust:test && bun run rust:check
```

There is no `lint` script at the moment. Add one only when the project has a real lint configuration.

## Git Hook

This repo uses `.githooks/pre-commit`.

Enable it once per clone:

```sh
git config core.hooksPath .githooks
```

Run it manually on Windows PowerShell when needed:

```sh
bash .githooks/pre-commit
```

The hook:

- runs `git diff --cached --check`
- blocks likely kubeconfig, token, certificate, or credential material
- enforces file-size caps from [handbook/file-size-and-split.md](handbook/file-size-and-split.md)
- runs `bun run typecheck` when available
- runs `bun run lint` when a lint script exists
- runs `cargo check` when a Rust manifest exists

## Testing Standard

Tests should prove behavior, contracts, or invariants. Do not add tests that only mirror implementation branches.

Use example-based tests for regressions and user-visible behavior. Use fixture-contract tests when Kubernetes object shape, Tauri serde contracts, or frontend wrapper boundaries can drift. Fixtures must be sanitized, minimal, and reusable.

Use property-based tests for pure deterministic logic with compact invariants, such as topology graphs, grouping, sorting, filtering, and cache-key normalization. Frontend property tests use `fast-check` with `bun test`; Rust property tests use `proptest`.

Do not require property tests for React rendering, live cluster integration, Tauri command tests that need real Kubernetes clients, or one-off bug examples.

## Verification Before Completion

Run the checks that match the work:

- docs-only: targeted Markdown/link/wording checks and `git diff --check`
- frontend: `bun run typecheck` and relevant `bun test`
- backend: `bun run rust:check` and relevant `bun run rust:test`
- behavior change: the nearest focused test
- Tauri integration: `bun run tauri dev` or build/smoke test, depending on scope
- release change: `bun run release:dry-run`
- guarded operation change: ADR 0003 or ADR 0004 checklist plus the nearest frontend/backend checks

If a check cannot run locally, record the exact blocker and what remains unverified.
