# Agent Guide

This repository is for KubeCove, a local desktop Kubernetes workspace built with Tauri v2, Svelte, TypeScript, Bun, Rust, and `kube-rs`.

## Current Frontend

KubeCove uses Svelte as its frontend runtime.

### Bits UI changes require explicit permission

- Do not modify, remove, replace, or override Bits UI primitives, their styling or behavior, or local shared wrappers under `src/components/ui/svelte/` without the user's explicit permission for that change.
- A general request to fix or improve a screen or tooltip does not authorize changing shared Bits UI components. Explain the proposed shared change and obtain permission first.
- Use the existing public component API for feature-level changes. Do not bypass this rule with global CSS, dependency patches, replacement components, or indirect overrides.

- Default frontend work should fit the Svelte app and preserve typed Tauri command boundaries.
- Prefer shared typed helpers/models where they reduce drift, but avoid new abstractions unless they are needed for parity with product behavior or testability.

Before making implementation changes, skim the [docs index](docs/README.md). The minimum reading set is:

- `docs/product-and-architecture.md`
- `docs/milestones.md`
- `docs/development-workflow.md`
- `docs/decisions/` (current ADRs)

## Core Constraints

- Keep the current beta inspection-first outside ADR-approved live-session or operation paths.
- Do not deploy anything into Kubernetes clusters.
- Do not expose raw kubeconfig contents to the frontend.
- Do not let frontend code run arbitrary shell commands.
- Use `kube-rs` and the Kubernetes API for core list/get/discovery/watch flows.
- Pod and selector-backed Service port-forwarding follow ADR 0003. Broader cluster-changing workflows must follow ADR 0004: typed Rust-side commands, visible target scope, confirmation where needed, and permission-aware UX.
- Treat `kubectl`, Helm, and Argo CD CLIs as future optional sidecars or fallbacks, not as the core data path.
- Treat Argo CD as a native product area. Kubernetes CRDs and tracking metadata remain the default inspection path; opt-in connected HTTP API inspection and operations follow ADR 0013.
- Keep modules small and typed so future agent work can target focused files.

## Preferred Architecture

Rust-side Tauri commands own Kubernetes access. Current command families include:

- `list_kube_contexts`
- `list_namespaces`
- `list_resource_kinds`
- `list_resources`
- `list_dynamic_resources`
- `list_resource_scope`
- `get_resource_yaml`
- `get_resource_details`
- `list_resource_events`
- `list_resource_topology`
- `list_resource_metrics`
- stream commands for watches, events, and pod logs
- live-session commands for Pod and selector-backed Service port-forwarding
- guarded selected-resource YAML apply plus typed scale, rollout-restart, and exact-delete commands
- explicit connected Argo CD discovery, connection, inspection, comparison, preflight, and operation commands
- Argo CD, Helm, RBAC, and usage commands

The frontend apps should call only typed Tauri command wrappers. Kubernetes credentials and kubeconfig parsing belong in Rust modules under `src-tauri/src`.

## File Organization

The engineering handbook at [docs/handbook/](docs/handbook/) is the source of truth for code organization, file size, and hygiene. Skim its [README](docs/handbook/README.md) before making structural changes.

Quick rules:

- New files: feature-specific code goes in `src/features/<area>/`; generic reusables in `src/components/`; pure logic in `src/lib/`. Backend commands go in `src-tauri/src/commands/<domain>.rs` or a focused domain folder. Module READMEs at each folder describe what belongs where.
- File-size caps: `.rs` 500 soft / 800 hard; `.svelte` 400 soft / 700 hard; `.ts` 300 soft / 600 hard. The pre-commit hook warns at soft, fails at hard. See [file-size-and-split.md](docs/handbook/file-size-and-split.md).
- Orphan and superseded files are deleted in the PR that creates the orphan. No "for reference" copies, no stub re-exports. See [hygiene.md](docs/handbook/hygiene.md).
- A new top-level directory requires a one-line entry in [code-organization.md](docs/handbook/code-organization.md) before the directory is created.

Run through [docs/handbook/pr-checklist.md](docs/handbook/pr-checklist.md) before claiming work complete.

## Implementation Style

- Prefer stable, well-maintained libraries with strong ecosystem support. Avoid niche or experimental dependencies unless they clearly solve a project-specific problem.
- Use Bun for frontend package management, scripts, and app scaffolding unless a tool requires a different runner.
- Commit `bun.lock` when frontend dependencies exist.
- Do not introduce duplicate router, table, state, styling, or command-wrapper libraries without an ADR.
- Keep changes reviewable and scoped.
- Use clear type names that match the docs unless there is a good reason to change them.
- Add comments only for non-obvious architecture decisions.
- Add TODOs for future features without implementing them early.

## Testing

- Never write unit tests after writing the code. They restate the implementation, always pass, and break on every refactor.
- Prefer E2E tests (`e2e/specs/`, see `docs/development-workflow.md`) as the primary way to verify features. Each E2E run must end with a verifiable, repeatable artifact under `e2e/artifacts/` (WDIO spec report, cluster diagnostics, failure screenshots).
- If a system must be tested in isolation (pure parsing, diff, topology, retry, or security-boundary logic), first write down every way it could fail, then write the code and tests against those failure modes.
- Delete unit tests that would not catch a real bug the E2E suite misses.
- Equal tests: the Svelte and GPUI clients share their UI acceptance tests in `e2e/parity/scenarios/`, so neither UI drifts from the other. A UI change in either client lands with a shared scenario that both drivers pass in `bun run e2e:parity`. Every view in `e2e/parity/visual/views.ts` needs a scenario whose ID starts with `<view>.`, and a new action needs a handler in both drivers, written in the area files described under GPUI work. `e2e/parity/contract.test.ts` enforces both. Native-only tests in `src-tauri/gpui/tests/native.rs` cover what the browser mocks cannot express, such as read errors, cancellation and stale reads. See `e2e/parity/README.md`.

## GPUI work

The native client lives in `src-tauri/gpui`. These rules come from parallel agent work on it.

- Give every checkout its own build directories, from the checkout root: `CARGO_TARGET_DIR=$PWD/src-tauri/gpui/target CARGO_BUILD_BUILD_DIR=$PWD/src-tauri/gpui/build`. Path crates get the same artifact names in every checkout, so a shared directory lets one build overwrite another's crates ("multiple different versions of crate kubecove_kube").
- On the shared workstation, run cargo as `nice -n 10` with `RUSTC_WRAPPER=` empty, and wrap builds, tests and parity runs in `flock <coordinating checkout>/src-tauri/gpui/target/heavy.lock`. Child checkouts use the lock path from their brief, so every agent queues on one lock.
- Do not set `CARGO_BUILD_JOBS` for GPUI builds. The job count comes from the machine's cargo configuration, and on the shared workstation mbx admits compilers from one pool with a memory budget. One job doubled the debug build after an edit and makes a release rebuild take minutes. See "GPUI client edit loop" in `docs/rust-build-measurements.md`.
- Cargo on that workstation resolves to the `mbx` shim, which shares compiled work across checkouts. A set `RUSTC_WRAPPER` bypasses it.
- Keep build outputs, binary copies, logs and ledgers inside the checkout, under `src-tauri/gpui/target/` or `e2e/artifacts/`. Never put them in `/tmp`.
- The parity work ledger is `src-tauri/gpui/target/plan.md` in the coordinating checkout. `target/` is ignored, so the ledger is not committed.
- `bun scripts/gpui-visual.ts [--skip-web] [view,view]` captures web and GPUI views from the same mock data and writes stacked images and a per-view RMSE to `e2e/artifacts/gpui-visual-<id>/`. `bun scripts/gpui-fonts.ts` regenerates the bundled static font cuts.
- To open the native client for the user, run `scripts/gpui-feel.sh`, and only when the user asks. It builds a release binary under the lock, copies it to `src-tauri/gpui/target/feel/`, and starts it with `systemd-run --user`, passing PATH and the display variables.
- Parity actions go in area files: `e2e/parity/web-<area>.ts` and `src-tauri/gpui/tests/parity/<area>.rs`. The hub files `src-tauri/gpui/src/inspector.rs`, `src-tauri/gpui/tests/parity.rs`, `e2e/parity/web.ts`, `e2e/parity/contract.ts` and `src-tauri/gpui/tests/common/mod.rs` get only a dispatch or registration line, so parallel branches do not conflict there.
- Resolve conflicts in `.rs` files by hand. Never union-merge them; keeping both sides duplicates items and breaks match arms.
- When a merge would push a file over its line cap, split the file first in its own commit (see `docs/handbook/file-size-and-split.md`).

## Performance Defaults

- Treat CodSpeed as the primary performance signal for deterministic hot-path logic, especially Rust backend code.
- Prefer CodSpeed-covered Rust benchmarks for backend logic that is local and repeatable, such as resource summarization, topology shaping, YAML/diff/serialization helpers, discovery or resource grouping, cache-key normalization, and command payload transformation.
- Do not benchmark live Kubernetes API calls directly in CodSpeed. Use Settings -> Diagnostics latency reports for real Tauri command, cluster, network, and webview paths.
- Frontend pure hot paths continue to use `bun run bench` and the existing CodSpeed Vitest benchmark suites.
- For perf-sensitive changes, measure the relevant path before and after, optimize one measured bottleneck at a time, and re-measure the same benchmark or diagnostic path.
- Do not copy iteration syntax blindly. In measured or repeated hot paths, inspect collection size, call frequency, allocation, and early-exit behavior; prefer one `for...of` pass over allocation-heavy spread or chained `map`/`filter`/`flatMap` when behavior stays exact. Keep `some`/`find`/`every` where their short-circuit semantics improve clarity, and never use `for...in` for arrays.
- Add a new benchmark only when the changed hot path is not covered by existing benchmarks or diagnostics.
- Fix a slow hot path by redesigning its data layout before adding a cache. Ask in this order:
  - Can a hash map keyed by IDs become a flat array, with references stored as indices into it? Can the most-used fields be packed into spare bits of that index?
  - Should this be a structure of arrays instead of an array of structures, so a pass touches only the fields it reads?
  - Can a short, heap-allocated list become a stack-allocated list with inline capacity?
  - Can an array or set of booleans become a bitset?
- Caching is a secondary tool. Add a cache only when a measurement shows the redesigned path is still too slow, and keep existing caches unless a redesign makes them unnecessary.

## Cross-Platform

KubeCove ships cross-platform: macOS (universal), Windows x64, and Linux x64 (see `.github/workflows/release.yml`). All changes must work on all three platforms, not just the one you are developing on.

- Never hardcode path separators, path-list separators, or home directories. Use `std::env::split_paths` for path lists (`:` vs `;`) and the existing helpers (e.g. `default_kubeconfig_path` handles `HOME` and `USERPROFILE`).
- Platform-specific code is gated with `#[cfg(target_os = "...")]` at the call site, but its helpers stay compiled on every platform so shared tests cover them — mark those helpers `#[allow(dead_code)]` with the standard comment (see `src-tauri/src/commands/usage_webview.rs`).
- Dead-code and lint results differ per OS: code that is "unused" on your machine may be the production path on another platform. Never delete platform-gated code because local clippy flags it; gate or annotate it instead.
- Frontend code must not assume platform-specific keyboard shortcuts (Cmd vs Ctrl), window chrome, or file-system casing behavior.
- The webview engine differs per platform: WKWebView/WebKit on macOS, WebView2/Chromium on Windows, WebKitGTK on Linux. Rendering-sensitive CSS (infinite animations, `filter`, compositing-heavy effects, especially inside transformed/scaled containers) must be verified on WebKit, not just on Windows — WebKit intermittently rasterizes animated-filter layers blank (see the topology glow fix).
- Prefer Tailwind utilities over bespoke CSS in `App.css`; reserve `App.css` for what utilities cannot express. This does not exempt engine bugs: Tailwind animations compile to the same CSS, so any infinite animation is a cross-platform risk. Keep them out of flow node subtrees, and prefer finite transitions or one-shot animations for state-change feedback.
- CI runs per-platform builds on release; local `cargo clippy`/`cargo test` only validates your current OS. Call out anything platform-sensitive in the PR so it gets attention on the other platforms.

## Tauri Command Contracts

- All frontend Kubernetes data must go through typed wrappers in `src/lib/tauri.ts`.
- Every new Tauri command needs a Rust serde model, a TypeScript type, and a typed frontend wrapper.
- Command errors must serialize into user-visible application errors.
- Do not add broad command payloads that leak kubeconfig, tokens, certificate data, or arbitrary filesystem contents.
- Live-session commands must follow [ADR 0003](docs/decisions/0003-guarded-live-sessions.md). Other cluster-changing commands must include explicit target scope and follow [ADR 0004](docs/decisions/0004-guarded-cluster-operations.md).

## Security-Sensitive Changes

Write an ADR before changing:

- the frontend/backend security boundary
- the JavaScript runtime or package manager
- the Kubernetes access path
- Tauri plugin permissions or capabilities
- guarded operation support for Kubernetes objects
- Argo CD API, CLI, sync, rollback, or diff integration beyond ADR 0013
- long-lived local persistence of cluster-derived data

## Product Inspiration

- Use K8Studio as a public feature benchmark for Kubernetes IDE capabilities.
- Use Aptakube as a public benchmark for clean, low-friction Kubernetes UX.
- Do not copy K8Studio or Aptakube code, assets, branding, proprietary layouts, or marketing text.
- Preserve this app's differentiator: context-first and namespace-first workflows.

## Verification

- Before launching or stopping KubeCove, check whether an app instance and its development server are already running. Reuse the existing instance by default; never close or restart a user-launched instance unless the user explicitly asks or a restart is technically required and has been explained first.
- Enable repo hooks with `git config core.hooksPath .githooks`.
- Run the relevant checks before claiming work is complete.
- Frontend changes should pass `bun run typecheck` and `bun run lint` (Biome).
- Rust backend changes should pass `cargo check --manifest-path src-tauri/Cargo.toml`.
- Kubernetes 1.34–1.37 is the tested rolling support window. ADR 0011 and the E2E/release workflow matrices are authoritative.
- If a check cannot run, state the exact blocker and what remains unverified.
- When completing scoped work, update the matching GitHub Issue. `docs/milestones.md` no longer holds a checklist.
- The pre-commit credential scan blocks `client-key-data:`, `client-certificate-data:`, `certificate-authority-data:` and `current-context:` in any file, and `clusters:`, `contexts:` or `users:` anywhere in YAML and config files. In code files those three keys match only alone on a line, so typed fields such as `contexts: Vec<String>` pass.

## Verification tiers

Run the smallest tier that covers the change, from the checkout root with the build settings under GPUI work. Pick the checks that would fail if the change broke, and stop there.

1. Agent patch from a parallel child agent: no compilation. The child runs no cargo command (check, clippy, test or build), no parity and no frames run. It edits, runs `bun run typecheck` and `bun run lint` for TypeScript changes, commits, and lists in its report what was never compiled or run. The parent merges the batch and runs tier 2 once for all of it. Never give a child `PARITY_CHANGED`: without the coverage map at `e2e/artifacts/parity-coverage-map.json` it runs the full suite. `e2e/parity/README.md` ("Change-based selection") shows how to download the map from CI or record it.
2. Merge of a batch into the working branch, run by the parent once per batch: `cargo clippy --manifest-path src-tauri/gpui/Cargo.toml --all-targets -- -D warnings`, `cargo test --manifest-path src-tauri/gpui/Cargo.toml --lib --test native`, and `PARITY_CHANGED=<working branch> bun run e2e:parity`.
3. After a fix: `PARITY_RERUN=last bun run e2e:parity` runs only the scenarios either driver did not pass in the newest run. Give an artifacts directory instead of `last` to use another run.
4. Full suite: CI runs the same change-based selection on the PR and the full suite after each merge to main (`.github/workflows/parity.yml`), so developers and agents run tiers 1 to 3 locally. Run `bun scripts/gpui-frames.ts --runs 3` only before a PR.

`PARITY_PREFIX` (comma-separated scenario ID prefixes) still narrows any run, such as one failing scenario or one area.

While iterating on one area, `cargo test --manifest-path src-tauri/gpui/Cargo.toml --test native <module>::` runs one module of `src-tauri/gpui/tests/native/`, such as `shell::`. The full native run takes about 14 seconds, and tier 2 still runs all of it.

Between PRs, run `bun scripts/gpui-frames.ts <case,case>` only for frame cases whose GPUI render or layout code changed.

Frontend changes in any tier also run `bun run typecheck`, `bun run lint` and `bun test e2e/parity/contract.test.ts`.

GPUI changes to column widths, toolbars or text sizes also run `src-tauri/gpui/tests/text_fit.rs`, which checks text against the bundled fonts. The parity driver cannot, because every glyph there is 0.6 em wide. Its header gives the command.

The runner starts both drivers at once. It builds the parity test once and runs it in `PARITY_JOBS` processes (`KUBECOVE_PARITY_SHARD=<index>/<count>`) that take scenarios from a shared queue, and the web driver runs as many pages. The default is half the CPU count. `e2e/parity/README.md` shows how to replay one native shard.

## Delegation briefs

Rules for agents that hand work to child agents, from observed failures.

- Start each child from the current branch head. Before it reports, it rebases its own unpushed commits onto the latest head. Stale bases caused most merge conflicts.
- Tell children to run builds and tests in the foreground, or to call `t3_background_wait` before ending a turn. Never end a turn with a job pending: a child that does is not woken when the job finishes, and its work sits uncommitted.
- Every brief names a stop condition and a first report point, such as "report after the first measured run".
- Parallel children compile nothing (tier 1). Give build settings and the lock path only to a single agent that runs a batch gate.
- When a child reports that it is waiting, check its worktree for finished commits before resuming it.

## Skill Backlog

- Use `docs/agent-skills.md` as the source of truth for project-specific skill ideas.
- Do not create or install actual Codex skills from that backlog without a focused skill-writing and validation pass.
- Until those skills exist, apply the documented rules manually when working in matching areas.

## Agent skills

### Issue tracker

Issues and PRDs are tracked in GitHub Issues for `Timpan4/kubecove`. See `docs/agents/issue-tracker.md`.

### Triage labels

Use the default five-label triage vocabulary. See `docs/agents/triage-labels.md`.

### Domain docs

This is a single-context repo; use the agent guide, product/architecture docs, and ADRs as the domain sources. See `docs/agents/domain.md`.

## Git Hygiene

- Use small commits.
- Avoid unrelated formatting churn.
- Do not rewrite user changes.
- Prefer branch names with the `codex/` prefix for agent-created feature branches.

## GitButler Workflow

Before any git write, run `git branch --show-current`.

If the current branch is `gitbutler/workspace`:

- Use GitButler mode.
- All git writes must use `but`; never use raw `git add`, `git commit`, `git push`, `git checkout`, `git rebase`, `git merge`, or `git stash`.
- Use `but diff --no-tui` for diffs.
- Never leave the workspace branch.
- If `but` is missing or unclear, run `but --help`; do not fall back to raw git writes.
- Before `but` commit, disable credit with `git config --local gitbutler.gitbutlerCommitter 0` unless requested otherwise.

Outside GitButler mode, normal non-destructive git rules apply. Do not force push, amend, rewrite history, or bypass force-push protection unless explicitly requested and safe.

Branch and commit naming:

- Branch names use `feat|fix|refactor|docs|test|chore|perf` plus a concrete 2-6 word kebab-case slug.
- Avoid vague names such as `tmp`, `misc`, `updates`, `stuff`, or ticket-only names.
- Do not add AI or coauthor attribution unless requested.
- Commit subjects should name one concrete outcome. An optional body may include 1-3 lines explaining why.
