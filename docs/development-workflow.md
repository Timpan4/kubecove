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

## Verification Before Completion

Before claiming a task is complete, run the checks that prove it:

- docs-only change: pre-commit hook or a targeted docs sanity check
- frontend change: `bun run typecheck` and `bun run lint`
- Rust backend change: `cargo check --manifest-path src-tauri/Cargo.toml`
- behavior change with tests: the relevant test command
- Tauri integration change: app dev or build command, depending on scope

If a check cannot run locally, say exactly why and what remains unverified.
