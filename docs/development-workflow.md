# Development Workflow

## Package Manager

Use Bun for frontend dependencies, scripts, and scaffolding.

Common commands:

```sh
bun install
bun run tauri dev
bun run typecheck
bun test
bun run bench
bun run rust:bench
bun run rust:test
bun run rust:check
bun run rust:lint
bun run check
```

## Browser Dev Mock Mode

`bun run tauri dev` starts the Vite server at `http://localhost:1430`. Opening
that URL in a normal browser during development runs the Svelte frontend with
browser-only mock Tauri responses. This is intended for Chrome DevTools,
browser automation, and frontend inspection.

The Tauri webview still uses real Tauri IPC and real Rust-side Kubernetes
commands. On Windows, `bun run tauri dev` exposes that development webview's
Chrome DevTools Protocol endpoint at `http://127.0.0.1:9222`. Attach Chrome,
Playwright, or another CDP client to inspect the real frontend console, network,
DOM, and runtime state. Set `KUBECOVE_DEVTOOLS_PORT` before launch to use a
different port. The endpoint is not enabled by `tauri build` or packaged apps.

On Windows, the same launcher stores Cargo development output under
`%LOCALAPPDATA%\KubeCove\cargo-target`. This keeps Rust incremental state off
ReFS Dev Drives, where transient file handles can prevent rustc from finalizing
an incremental compilation session. An explicit `CARGO_TARGET_DIR` still wins.

The browser tab does not receive kubeconfig contents, does not call a
local Rust bridge, and does not access a real cluster. Treat all browser data as
fake, even when a saved workspace name matches a real context.

## Deterministic E2E and Kind Lab

Use the fast suite while changing frontend behavior:

```sh
bun run e2e:fast
```

It starts Vite, opens Chrome through WDIO, and reuses the typed DEV browser mocks. It does not build Rust or contact Kubernetes.

Use the real suite for native command and cluster behavior:

```sh
bun run e2e:real -- --kubernetes 1.35 --provider auto
bun run e2e:real -- --keep
bun run e2e:cleanup -- --run-id <id>
```

The runner downloads checksum-verified pinned tools into its cache, creates a uniquely named Kind cluster through Docker or Podman, applies the deterministic lab, builds the E2E-only Tauri flavor, and runs native WDIO serially. It gathers redacted diagnostics before deleting its exact cluster on success, failure, SIGINT, or SIGTERM. Local `--keep` skips automatic cleanup so the exact run can be inspected; `--keep` is rejected in CI. Cleanup always needs an exact recorded run ID; it never deletes clusters by prefix.

The generated real-suite kubeconfig contains only dedicated admin and restricted contexts. E2E startup rejects non-absolute paths, unexpected contexts, non-loopback API servers, cluster-name mismatches, persisted sources, and fallback to the user's default kubeconfig. Artifacts never include raw kubeconfig, tokens, keys, or certificate data.

For manual development against the same fixtures:

```sh
bun run dev:kind
bun run dev:kind:down
```

`dev:kind` creates or reuses a cluster named from a hash of the workspace path, reapplies fixtures idempotently, and launches normal Tauri development with a temporary settings/WebView profile. The app profile disappears when the command exits; the cluster remains until `dev:kind:down`. An existing Podman machine may be started, but the command does not stop it.

Native desktop launch/settings smoke without Kind is available through:

```sh
bun run e2e:desktop-smoke
```

Kubernetes 1.34–1.36 is the current rolling, tested three-minor window. Advancing it requires digest-pinned Kind images and a green full matrix as defined by [ADR 0011](decisions/0011-rolling-kubernetes-support.md). The E2E-only desktop permission boundary is defined by [ADR 0010](decisions/0010-e2e-only-wdio-security-boundary.md).

`bun run check` runs the current local verification bundle:

```sh
bun run typecheck && bun test && bun run rust:test && bun run rust:check && bun run rust:lint
```

`bun run rust:lint` runs Clippy for all Rust targets and features with warnings denied.

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

Do not require property tests for UI rendering, live cluster integration, Tauri command tests that need real Kubernetes clients, or one-off bug examples.

## Performance Checks

CodSpeed is the preferred performance signal for deterministic hot-path logic. Current frontend CodSpeed coverage runs through Vitest:

```sh
bun run bench
```

Existing frontend suites cover resource table modeling and ownership inheritance,
sidebar tree building, topology selection and layout at 1k and 4k nodes, resource
metric merging, command-palette indexing, YAML dry-run diff generation, resource
detail logs/events/timelines, RBAC cockpit shaping, GitOps tables and summaries,
and Helm grouping and reconciliation sorting.

Backend CodSpeed coverage runs through `codspeed-divan-compat`:

```sh
bun run rust:bench
```

Existing backend suites cover Helm manifest parsing and summarization, backend YAML and KYAML serialization, ownership topology building, CRD catalog sorting, and present-custom-resource scope key generation. Backend performance work should prefer CodSpeed-covered Rust benchmarks when the hot path is deterministic and local, such as resource summarization, topology shaping, YAML/diff/serialization helpers, discovery or resource grouping, cache-key normalization, and command payload transformation.

Do not use CodSpeed for live Kubernetes API timing. Use Settings -> Diagnostics latency reports for real Tauri command latency, cluster/network behavior, and webview paths. Keep `bun run perf:frontend` and `bun run perf:resource-scope` as local comparison helpers for app-level or resource-scope experiments that do not fit the benchmark suites.

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
