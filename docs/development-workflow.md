# Development Workflow

## Package Manager

Use Bun for frontend package management, scripts, and scaffolding.

Expected frontend commands after the app is scaffolded:

```sh
bun install
bun run dev
bun run typecheck
bun run lint
```

Rust checks should run through Cargo:

```sh
cargo check --manifest-path src-tauri/Cargo.toml
```

## Pre-Commit Hook

This repo uses a checked-in Git hook at `.githooks/pre-commit`.

Enable it once per clone:

```sh
git config core.hooksPath .githooks
```

On Windows PowerShell, run the hook manually with:

```sh
bash .githooks/pre-commit
```

The hook currently:

- runs `git diff --cached --check`
- blocks likely kubeconfig or credential material
- enforces file-size caps from [handbook/file-size-and-split.md](handbook/file-size-and-split.md): warn at soft cap, fail at hard cap; legacy oversized files exempt via an explicit list in the hook
- runs `bun run typecheck` when `package.json` defines `typecheck`
- runs `bun run lint` when `package.json` defines `lint`
- runs `cargo check` when a Rust manifest exists

The hook is intentionally lightweight before the app scaffold exists. Once the Tauri project is created, keep the hook aligned with the real scripts in `package.json`.

## Testing Standard

Use example-based tests for specific regressions and user-visible behavior. Add fixture-contract tests when Kubernetes object shapes, Tauri serde contracts, or frontend wrapper boundaries can drift. Fixtures must be sanitized, minimal, and checked into `tests/fixtures/` only when they represent a reusable Kubernetes or app contract.

Use property-based tests for pure deterministic logic with compact invariants and many input combinations, such as topology graphs, grouping, sorting, filtering, and cache-key normalization. Keep generated inputs small, deterministic, and focused on invariants. Do not require property tests for React rendering, real cluster integration, Tauri command tests that need live Kubernetes clients, or one-off bug examples.

Frontend property tests use `fast-check` with `bun test`. Rust property tests use `proptest` as a dev dependency. Prefer fixture-contract tests first when the risk is external shape drift; add property tests when the risk is combinatorial behavior.

## Verification Before Completion

Before claiming a task is complete, run the checks that prove it:

- docs-only change: pre-commit hook or a targeted docs sanity check
- frontend change: `bun run typecheck` and `bun run lint`
- Rust backend change: `cargo check --manifest-path src-tauri/Cargo.toml`
- behavior change with tests: the relevant test command
- Tauri integration change: app dev or build command, depending on scope

If a check cannot run locally, say exactly why and what remains unverified.
