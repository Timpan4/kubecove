# Development Workflow

## Package Manager

Use Bun for frontend dependencies and scripts, at the version CI pins. Dependency versions belong in lockfiles and manifests, not in this guide.

```sh
bun install
bun run tauri dev
bun run typecheck
bun test
bun run docs:check
bun run check
```

## TypeScript and Svelte checks

`typecheck` and `typecheck:bench` use tsrs. `svelte:check` uses `svelte-check-native` for Svelte transformation and tsrs for type checking. TypeScript 6 and the official `svelte-check` package are not installed.

Every supported platform uses the published tsrs binary for its platform. `TSRS_BINARY` selects another existing binary, and is required on a platform without a published one.

`scripts/svelte-check.mjs` combines native diagnostics with CSS validation from `vscode-css-languageservice`. It uses the native compiler's file graph to cover the same components and preserve source positions. Svelte's compiler bridge preserves accessibility and unused-selector warnings. The wrapper supports `--workspace`, `--tsconfig`, `--fail-on-warnings`, and `--output human-verbose|machine-verbose`. Watch mode is not supported. `TSGO_BIN` is set automatically from the selected tsrs binary.

After upgrading either checker, run `bun run test:svelte` plus the project TypeScript and Svelte checks. The integration regression exercises component type and prop errors, original source positions, accessibility and CSS warnings, scriptless components, component deletion, warning exit status, and compiler-process failure.

## Linux native dependencies

Debian/Ubuntu native builds and headless desktop checks use the same packages as
Linux CI:

```sh
sudo apt-get update
sudo apt-get install -y build-essential pkg-config dbus dbus-x11 gnome-keyring libayatana-appindicator3-dev libsecret-1-0 libwebkit2gtk-4.1-dev librsvg2-dev libssl-dev patchelf xvfb xauth
```

For example, missing `glib-2.0.pc` is a system dependency failure, not a Rust
source error. Verify with `pkg-config --modversion glib-2.0 gtk+-3.0 webkit2gtk-4.1`.

On small Linux VMs, Oxlint's JavaScript-plugin allocator can fail to reserve its
large aligned block ([upstream issue #20331](https://github.com/oxc-project/oxc/issues/20331)).
`lint:anti-slop` retries only that specific crash with jemalloc preloaded into the
linter process. It preserves all rules and the retry's exit status; the app and
profiler do not inherit the allocator. Install `libjemalloc2` on Debian/Ubuntu,
or extract the distribution package without root:

```sh
mkdir -p .e2e/tools/allocator
(cd .e2e/tools/allocator && apt-get download libjemalloc2 && dpkg-deb -x libjemalloc2_*.deb root)
bun run lint:anti-slop
```

## Rust workspace checks

The desktop manifest is a workspace containing private backend crates under `src-tauri/crates/`. Use `bun run rust:test`, `bun run rust:check` and `bun run rust:lint` for workspace-wide checks, including tests moved into dependency crates. Direct equivalents are:

```sh
cargo test --locked --workspace --manifest-path src-tauri/Cargo.toml
cargo check --locked --workspace --manifest-path src-tauri/Cargo.toml
cargo clippy --locked --workspace --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```

For a backend-only check, add `--no-default-features`; for E2E source isolation, use the shell's `--features e2e`. The default package remains the desktop shell so existing Tauri build and packaging commands keep their output paths. See [the crate map](../src-tauri/crates/README.md) and [build measurements](rust-build-measurements.md).

## GPUI native client development

The opt-in native client is a separate Cargo workspace under `src-tauri/gpui/`. It uses GPUI Kit and calls the shared backend with Tauri features disabled. The Svelte and Tauri application builds keep their existing dependencies. See [ADR 0029](decisions/0029-gpui-native-client-and-feature-parity.md) and the [native client guide](../src-tauri/gpui/README.md) for scope and platform prerequisites.

```sh
cargo run --locked --manifest-path src-tauri/gpui/Cargo.toml
cargo check --locked --manifest-path src-tauri/gpui/Cargo.toml
cargo fmt --manifest-path src-tauri/gpui/Cargo.toml --all -- --check
cargo clippy --locked --manifest-path src-tauri/gpui/Cargo.toml --all-targets -- -D warnings
bun run e2e:parity
```

The parity command exercises both production interfaces with controlled backend responses using the same [acceptance scenarios](../e2e/parity/scenarios/) and the same exported browser-mock data. It retains reports and screenshots when available under `e2e/artifacts/`. This is UI integration evidence, separate from packaged-window and real Kubernetes checks. The full suite runs in CI on every pull request and after each merge to main (`.github/workflows/parity.yml`); run it locally with `PARITY_PREFIX` for the areas you change. Unsupported native product areas remain listed in the [parity guide](../e2e/parity/README.md).

When a user-level Cargo configuration sets a shared build directory, override both `CARGO_TARGET_DIR` and `CARGO_BUILD_BUILD_DIR` for isolated builds. The target directory alone does not override Cargo's intermediate build directory.

## Headless Server Development

The server is a separate Rust binary under `src-tauri/server`; it uses the shared backend without enabling the desktop feature. Build the browser runtime before compiling the server so the binary embeds the generated `dist/` assets:

```sh
KUBECOVE_PUBLIC_RUNTIME=web bun --bun vite build
cargo build --locked --release --manifest-path src-tauri/server/Cargo.toml
```

The headless build does not require Tauri, a webview, or Linux desktop GUI development packages. Its supported release targets are Linux x64 and Linux arm64. It also builds from source on macOS, but releases do not publish macOS server archives. The release workflow builds and archives each target separately.

For an API-only server, run `cargo build --locked --release --no-default-features --manifest-path src-tauri/server/Cargo.toml`. This disables the default `web-ui` feature and needs neither Bun nor `dist/`. Run server tests and Clippy with `--no-default-features` to check this variant. The release workflow also builds and smoke-tests the full and slim Linux x64 container images, then publishes those tested artifacts to GHCR after the release gates pass. See [the container guide](../deploy/README.md) for targets and version tags.

Run isolated server checks with `cargo test --locked --manifest-path src-tauri/server/Cargo.toml` and `cargo clippy --locked --manifest-path src-tauri/server/Cargo.toml --all-targets -- -D warnings`. The server verification contract and runtime E2E evidence are maintained separately from this packaging guide; building or testing the binary alone does not prove browser or desktop remote E2E behavior. See [the remote-host operations guide](wiki/Remote-Server.md) for listener, SSH, token, and data-directory use.

## Browser Dev Mock Mode

`bun run tauri dev` starts Vite on Bun at `http://localhost:1430`. Opening it in a normal browser runs Svelte with browser-only mock Tauri responses for frontend inspection and automation.

The Tauri webview uses real IPC and Rust Kubernetes commands. On Windows, development does not expose Chrome DevTools Protocol by default. Set `KUBECOVE_DEVTOOLS=1` before `tauri dev` to expose it at `http://127.0.0.1:9222`, and set `KUBECOVE_DEVTOOLS_PORT` to change the port. Packaged apps do not expose this endpoint.

Browser mock mode never receives kubeconfig contents, calls a local Rust bridge, or accesses a real cluster. Treat all browser data as fake.

To drive a feature in mock mode and keep evidence, follow the [`verify-kubecove` skill](../.agents/skills/verify-kubecove/SKILL.md). It runs Vite on a free loopback port with headless Chrome, so it never touches a server on port 1430.

For an optional Playwright smoke check against Obscura, install the rendering-enabled binary from the [Obscura releases](https://github.com/h4ckf0r0day/obscura/releases) and make `obscura` available on PATH. Start Obscura and Vite in separate terminals, then run the check:

```sh
obscura serve --port 9223 --allow-private-network
bun run dev
bun run e2e:obscura
```

The command uses Playwright's `chromium.connectOverCDP` to drive Obscura. It checks the rendered workspace form, mock context, and text input. It writes `page.txt` and `result.json` to a new ignored directory under `e2e/artifacts/`. Set `OBSCURA_CDP_URL` if Obscura runs on another local port. Port 9223 avoids KubeCove's development CDP port. This checks only the browser mock frontend. Use the WDIO suites below for desktop and Kubernetes behavior.

## Deterministic E2E and Kind Lab

Use fast E2E while changing frontend behavior:

```sh
bun run e2e:fast
```

It starts Vite on Bun, runs the Playwright/Obscura smoke check, then runs the existing Chrome/WDIO scenarios with typed development-browser mocks. The runner downloads a checksum-verified Obscura release and stops only the Obscura process it started. Port 9223 must be free. Each run writes its result, Obscura server log, smoke results, and WDIO reports under a new `e2e/artifacts/fast-<run-id>/` directory. Startup failures also write a result artifact. It neither builds Rust nor contacts Kubernetes.

Use real E2E for native-command and cluster behavior:

```sh
bun run e2e:real -- --provider auto
bun run e2e:real -- --keep
bun run e2e:cleanup -- --run-id <id>
```

The runner downloads checksum-verified tools and assets, creates one uniquely named Kind cluster, builds the E2E-only Tauri flavor, and runs native WDIO serially. It installs real Cilium, Git, Argo CD, Helm, and tenant fixtures. It does not use fake GitOps CRDs or fabricated status.

The runner gathers allowlisted, redacted diagnostics, then deletes only its own cluster on success, failure, SIGINT, or SIGTERM. A local `--keep` preserves that run for inspection and is rejected in CI. Cleanup requires the recorded run ID. It never deletes by prefix or reads the user kubeconfig.

The generated E2E kubeconfig contains only dedicated contexts. Startup rejects non-absolute paths, unexpected contexts, non-loopback API servers, cluster mismatches, persisted sources, and fallback to the user default kubeconfig. Artifacts never include raw kubeconfig, tokens, keys, or certificate data.

For manual development against the same fixtures:

```sh
bun run dev:kind
bun run dev:kind:down
bun run e2e:desktop-smoke
bun run e2e:desktop-smoke -- --spec e2e/specs/desktop/exec-plugin.e2e.ts
```

The focused exec-plugin scenario uses an isolated native app and a controlled
credential subprocess. It checks IPC responsiveness and workspace cancellation
without a cluster or real provider login. The desktop runner accepts WDIO's
`--spec` selection; artifacts still go under `e2e/artifacts/desktop-<run-id>/`.

`dev:kind` uses a cluster name derived from the checkout and a temporary app profile. The cluster stays until `dev:kind:down`. The app profile is removed on exit.

See [ADR 0010](decisions/0010-e2e-only-wdio-security-boundary.md), [ADR 0011](decisions/0011-rolling-kubernetes-support.md), and [ADR 0012](decisions/0012-production-shaped-e2e-lab.md).

## CI caching

CI uses standard GitHub-hosted Ubuntu runners. The main CI workflow uses
[Mr Boxington](https://mr-boxington.jdx.dev/github-action) for Rust tests, Clippy,
and headless server builds. The action and mbx version are pinned in the workflow.
Each task has its own cache and an explicit Cargo workspace, covering
`src-tauri`, `src-tauri/server` and `src-tauri/gpui`. The Parity and CodSpeed
workflows cache their Rust builds the same way.

CI uses the action's `objects` payload with one isolated store per job.
Cargo 1.100 moved unit state under `build/<package>/<hash>/`; the pinned action's
`target` payload scanner skips that subtree and refuses to save it. Object
export/import avoids that scanner. The post step cleans the consumed target before
exporting the completed commands' compiler objects, then removes its isolated
store before uploading the bundle. The cache keys share no prefix with the earlier
target snapshots, so enabling clean-before-export never imports one, as required by
[the upstream migration caveat](https://github.com/jdx/mr-boxington-action/issues/91).

`scripts/ci-rust-cache-key.mjs` writes each cache key. The key names the compiler,
the runner image and the Git blob of every tracked file the build reads, including
the workflow file. A build whose inputs did not change restores its exact cache,
and the action then skips the export and upload. When every run saved, that step
took 14 to 200 seconds per job, and one pull request run stored 6 GB of the
repository's 10 GB cache quota. A changed build restores the newest cache with
the same compiler and rebuilds what changed. Only default-branch pushes save.
Pull requests restore main's cache and never save: one pull request push that
saved stored about 7 GB, and GitHub evicts the least recently used caches above
the quota, which would remove the caches main keeps for other pull requests.
Jobs that share one cache key, such as the CodSpeed Rust shards and the main
parity shards, let one job save, because the action fails a job whose save loses
GitHub's reservation of the key to another job. The others set GitHub's
`cache-mode: read` on the job, which gives them read-only cache access.
`cache-mode` takes one literal value per job, so the saving CodSpeed shard is the
`rust-saver` job, and in `parity.yml` the spike job saves for the main shards.
Setting `ACTIONS_CACHE_MODE` in a step's `env` does nothing: the runner sets that
variable from the job's `cache-mode`. Each of these jobs prints the access it got
in its "Show cache access" step. The pull request parity jobs are read-only too,
so every pull request exercises that path before main depends on it.

CI builds set `CARGO_PROFILE_DEV_DEBUG=line-tables-only`. Backtraces keep file
and line, and the compiler output that each job links, caches and restores is
smaller. Builds run through `mbx`, while `cargo tree` remains a direct inspection
command. The Cargo shim also wraps Rust builds launched by Bun.

Change detection selects a verification matrix. It disables fail-fast so a
failed task does not cancel the remaining checks. Checks that build the same
workspace with the same settings share one task, so they share one setup and
one cache:

- `docs`: documentation and whitespace checks;
- `frontend-e2e`: `e2e:fast`, separate from the frontend checks and compiler
  integration tests in the `Frontend checks` job;
- `rust`: desktop Rust tests, Clippy and dependency audits;
- `gpui-clippy` and `gpui-test`: GPUI Clippy, and library and native tests;
- `server-backend`: GUI-free backend tests and dependency-tree verification;
- `server`: API-only server tests and Clippy, then the browser UI build,
  default-feature server tests and Clippy, the server build and HTTP/WebSocket
  lifecycle verification.

The default-feature server checks build the real browser assets that
`src-tauri/server/build.rs` embeds. The API-only checks run before that build,
and the GUI-free backend task never builds it, so both prove they need neither
those assets nor GUI packages. Native build prerequisites are installed
only when `pkg-config` cannot resolve them. Caching compiler outputs does not
supply missing system libraries.

Nix retains its existing push/manual gate and runs after the matrix, so it does
not exceed the same concurrency budget. The required check remains
`Typecheck and test`. It requires successful change detection, the frontend job,
every selected matrix task and Nix job to pass. Unexpected skips, failures and cancellations
fail the gate. Same-repository labeled release PRs retain their existing skip
policy.

The short jobs target warm PR feedback. Hosted cold/warm runs must confirm the
cache transfer time, hit rates and wall time; local regression tests do not prove
a three-minute job or the overall three-to-four-minute goal.

Standalone and release E2E use mbx with separate desktop and headless server
workspace caches. The primary Kubernetes version publishes the Linux E2E cache
on pushes to `main`; the scheduled matrix and release E2E restore it. Desktop
caches remain separate by OS and architecture. A manual E2E run on `main`
seeds the macOS and Windows caches; scheduled runs restore them.
The Cargo shim installed by
`scripts/ci-mbx-cargo.mjs` also wraps Cargo launched by Bun and Tauri.
`MBX_TARGET_VIEWS=0` preserves the target paths consumed by E2E and packaging.

Native desktop and headless server release builds use mbx's `local` backend.
They neither restore nor save shared compiler outputs, following the upstream
[production release guidance](https://mr-boxington.jdx.dev/github-action#production-releases).
Container and Nix builds keep their existing build paths. CodSpeed retains its
Rust dependency cache.

The frontend, headless server, and E2E jobs share Bun's download cache on each
OS. They still run `bun install --frozen-lockfile` after restoring it. Caches
never replace a build or test gate, and a cache miss runs the same checks.

PR runs skip Nix builds, including release PRs. Relevant pushes to `main` and
manual CI runs retain the Nix build gate. The daily Nix lock refresh builds its
packages before opening a PR, and release tags retain advisory Nix builds.

Nix currently rebuilds without a persistent store cache. Check the repository's
GitHub cache usage before adding one. A whole-store Nix cache can evict Rust
caches. Cache storage and billing settings are unchanged by the workflows.

To evaluate a CI change, compare job queue, setup, execution, and cache upload
times in GitHub Actions on comparable revisions. Include both cold and warm
runs. Local workflow regression checks validate orchestration, not hosted cache
hits or an end-to-end speedup.

## Verification Before Completion

Run the smallest checks that cover the work:

- docs-only: `bun run docs:check` and `git diff --check`
- frontend: `bun run typecheck` and nearest `bun test`
- backend: `bun run rust:check` and nearest `bun run rust:test`
- behavior change: nearest E2E spec (`bun run e2e:fast` or `bun run e2e:real`) or existing focused test; see the Testing rules in `AGENTS.md`
- Tauri integration: `bun run tauri dev` or smoke test
- release change: `bun run release:dry-run`
- guarded operation: relevant ADR checklist plus nearest frontend/backend checks

If a check cannot run, record the exact blocker and what remains unverified.

## Interaction recordings

Settings → Diagnostics → Record interactions measures each click, drag, key, and
scroll in the running webview: input-to-paint time, dropped frames against the
measured refresh interval, and DOM nodes added, removed, and changed, grouped by
source. Dev builds group by Svelte file and line. Release builds have no source
locations and fall back to `data-testid` attributes and ARIA roles, which the app
sets on few elements, so release attribution is coarse; profile in a dev build to
find the component. Enable diagnostics as well to include Tauri command
timings. Entries stay in memory, and Copy outputs JSON lines.

Under `bun run tauri dev`, the Vite server also appends each entry to
`e2e/artifacts/dev-interactions/<server-start>.jsonl`. `bun run perf:interactions
[file]` summarizes the newest recording, or a pasted copy. Measure on the target
webview: Chromium timings do not predict WebKitGTK behavior. Recording adds
overhead, so compare runs only with recording on in both.

## Startup and memory baselines

`bun run build` writes `.e2e/reports/frontend-bundle.json` outside the shipped
assets. `bun run perf:startup` builds the existing E2E desktop flavor in **release**
mode with the production minified frontend, then records an isolated launcher
scenario in `e2e/artifacts/desktop-<run-id>/startup.json`. Use `--run-id <id>` for an
explicit new run identifier. It creates no cluster, uses only the smoke runner's
synthetic loopback kubeconfig, and removes its temporary profile on completion.
It never reads the user's kubeconfig. Reports remain local and ignored by Git.

On headless Linux/WSL, run inside a disposable D-Bus/keyring and Xvfb session:

```sh
dbus-run-session -- bash -euc '
  eval "$(gnome-keyring-daemon --start --components=secrets)"
  dbus-send --session --print-reply --dest=org.freedesktop.secrets /org/freedesktop/secrets org.freedesktop.Secret.Service.SetAlias string:default objpath:/org/freedesktop/secrets/collection/session
  exec xvfb-run -a bun run perf:startup
'
```

The session keyring is temporary. On a small VM, set `CARGO_BUILD_JOBS=1` for the
build if necessary; record the setting when comparing build times.

The runner checks for KubeCove processes and occupied frontend/automation ports
before building and again before launch. It refuses an existing instance rather
than restarting it. The embedded WDIO service owns the app it starts; interrupt
cleanup signals only this run's process group (or exact process tree on Windows).
No production command, permission, or automation bridge is added. IPC timing is
compiled in only with `KUBECOVE_PUBLIC_PROFILE=true`; ordinary builds retain only
eight one-time User Timing marks. Runtime logs and screenshots are not profile
artifacts.

Readiness times are milliseconds from the WebView's `performance.timeOrigin`,
not from operating-system process creation. Marks record the first occurrence
per document and are reported in observed chronological order:

| Mark | Meaning |
| --- | --- |
| `frontend-entry` | The early entry dependency begins evaluation, before the Svelte application graph evaluates. Parsing/loading before this point is not isolated. |
| `svelte-mount` | Svelte's root `mount` returns with the initial DOM attached; this does not assert a painted frame. |
| `path-restored` | The app has read and applied the saved launcher/workspace path, or completed the no-saved-path branch. |
| `launcher-ready` | The launcher has successful kubeconfig-source and context results. It does not wait for namespace discovery in the creation form. |
| `workspace-ready` | The workspace shell mounts with its initial navigation state. This does not assert resource readiness. |
| `kubeconfig-ready` | The first successful typed `get_kubeconfig_sources` call settles. |
| `base-scope-ready` | The resource browser has source readiness and successful namespace and resource-kind discovery. |
| `first-resource-rows` | A visible table has non-placeholder successful resource data and at least one uncollapsed resource entry after the DOM update. Empty results and collapsed group headers do not count as rows. |

IPC summaries aggregate completed invocations by command name: count, errors,
total duration, and maximum duration. Time includes frontend serialization,
bridge scheduling, backend work, and response decoding; it is not a backend CPU
measurement. The first 1,000 completed calls are retained; `limitReached` exposes
truncation. Arguments, results, errors, resource names, and kubeconfig paths are
never recorded. The memory sample follows the launcher snapshot and uses the
existing `get_app_usage_metrics` contract. Host, WebView, and other-child resident
bytes are process groups, not allocation attribution; shared pages can be counted
more than once. Optional Chromium JS heap bytes are separate and must not be
added to process memory. Missing groups and unsupported heap inspection are
`null` with an explicit reason, never zero.

Bundle sizes include every emitted file, with input ownership and chunk imports.
Public assets are included alongside generated chunks and styles. Schema version 2
records source ownership without per-input byte estimates, which are not comparable
between bundlers. `rawBytes` is
emitted file size; gzip level 9 and Brotli quality 11 are deterministic per-file
compression comparisons, not installer sizes or measured network transfers.
Totals include deferred chunks. Reports exclude timestamps and absolute source
paths so identical builds can be compared directly.

Repeat measurements with the same source, lockfiles, build settings, OS, viewport,
and user path. Source SHA and dirty state are recorded. This initial scenario is
`launcher-fresh-profile`: it includes E2E automation overhead, does not flush OS
filesystem caches, and is not a packaged-installer or restored-workspace test.
Workspace, base-scope, and resource marks are explicitly `not-reached` here.
Do not interpret them as zero-cost or use this launcher run to choose a resource
retention policy. Windows and macOS/Linux WebKit results must be recorded
separately before making cross-platform performance claims.
