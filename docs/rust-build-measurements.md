# Rust crate split and build-turnaround evidence

## Scope

The desktop package now composes six private backend crates: models, Kubernetes/session infrastructure, resource inspection, live sessions, GitOps and host support. See [the crate map](../src-tauri/crates/README.md). Inspection, live sessions and host support can compile independently after their common prerequisites. GitOps depends on live forwarding, but not on resource inspection. The split changes compilation boundaries, not command payloads or runtime behavior.

The server remains a standalone workspace with a separate lockfile and target directory. Its normal/build dependency tree must remain free of Tauri and webview packages.

## Failure modes checked during extraction

- Cyclic dependencies through workspace cancellation, Argo connections, Flux kind metadata or Helm's dynamic projection helpers.
- Duplicated task-local session scopes, cancellation markers, client caches or credential state after moving modules.
- Desktop command macros or E2E source-isolation flags not propagating through crate features.
- Server feature unification accidentally enabling Tauri.
- Cargo tests/lints covering only the composition package and silently skipping relocated unit tests.
- Manifest-relative kubeconform paths and relative JSON fixtures resolving from the wrong directory.
- Benchmark adapters depending on crate-private fields/functions across the new boundaries.
- Release/source-filter paths or Markdown references continuing to point at removed files.

Existing contract, cancellation, source-isolation, forwarding, GitOps and server-parity tests are retained. Unit tests were not rewritten to mirror the new crate layout.

## Measurement protocol

Baseline (`d54282f`) and split use Windows x64 on an AMD Ryzen 7 9800X3D, Rust/Cargo 1.98.1, the existing default development profile and `RUSTC_WRAPPER=sccache`. No target, linker, LTO, codegen-unit or optimization settings were changed. The baseline was recorded before moving sources. Generated evidence stays under ignored `e2e/artifacts/rust-crate-split/`.

Workload:

```sh
cargo build --locked --manifest-path src-tauri/Cargo.toml --no-default-features
# After warmup, invalidate one representative source timestamp, then rebuild.
touch <source>
time cargo build --locked --manifest-path src-tauri/Cargo.toml --no-default-features
```

Three samples per source were collected. Sources are usage monitoring, typed core-resource summaries, and Argo Application summaries. After extraction, the corresponding files are under `crates/host/src/usage.rs`, `crates/inspection/src/commands/resources/summary_core.rs`, and `crates/gitops/src/commands/argo/applications.rs`.

This is a warm, timestamp-invalidated rebuild with unchanged contents, not a clean build or a historical semantic-edit benchmark. It measures Cargo/rustc incremental invalidation and final library linking. It does not establish desktop launch/build, release-build, macOS or Linux speedups. Initial dependency compilation, unrelated machine load and compiler caches can materially affect timings; do not compare initial warmup time with incremental samples.

## Results

All values below are shell elapsed seconds, three samples per source:

- Usage: baseline **9.900, 9.628, 8.398**; split full-library build **29.989, 34.507, 16.803**.
- Core-resource summaries: baseline **8.521, 8.644, 9.367**; split full-library build **17.976, 9.027, 17.666**.
- Argo Applications: baseline **8.759, 9.427, 8.201**; split full-library build **18.379, 16.724, 26.553**.

**These runs do not demonstrate an end-to-end compilation speedup.** The split full-library builds were slower, with substantial variance, on a machine concurrently running other build/benchmark work. The observations must not be presented as a 30–40% improvement or as a clean, controlled before/after comparison. Further same-load profiling is needed before making a full-app build-time claim. Release and cold-build effects remain unmeasured.

The narrower invalidation boundary is verified by Cargo's output:

- touching `host/src/usage.rs` rebuilt only `kubecove-host` and the root;
- touching the resource summary rebuilt only `kubecove-inspection` and the root;
- touching the Argo summary rebuilt only `kubecove-gitops` and the root.

Other domain crates were reused. The root still generates the original `staticlib`, `cdylib` and `rlib` artifacts; this work intentionally does not drop an artifact contract to make the crate-split comparison look better. A later `--timings` probe reported 11.51 seconds in the host crate and 18.26 seconds in the root, with 40.05 seconds total, reinforcing the need to profile full artifact generation separately rather than assume the split alone solves turnaround.

### Focused development workflow

The split also makes domain-only builds possible. After warmup:

```sh
cargo build --locked --manifest-path src-tauri/Cargo.toml --no-default-features -p kubecove-host
cargo check --locked --manifest-path src-tauri/Cargo.toml -p kubecove-inspection
cargo test --locked --manifest-path src-tauri/Cargo.toml --no-default-features -p kubecove-live
```

A timestamp-invalidated host-only build took **4.717, 2.500, 4.530 seconds**, and compiled only `kubecove-host`. This is a smaller workload that skips the final application artifacts, **not an apples-to-apples speedup for the full app**. Use focused package checks while iterating, then the workspace gates before completion.

## Verification

- Workspace default tests: **369 passed**, two existing ignored fixtures.
- Workspace GUI-free library tests (`cargo test --workspace --no-default-features --lib`): **361 passed**, one existing ignored subprocess fixture (launched by its parent test).
- Focused infrastructure tests with `e2e` enabled: **78 passed**, including loopback kubeconfig isolation and context validation.
- API-only server tests: **66 passed**, two existing ignored fixtures.
- Workspace check and all-target/all-feature Clippy with `-D warnings`: passed, including E2E/benchmark compilation and cross-crate Tauri command macros.
- Server normal/build dependency tree: no Tauri, GTK, Wry or WebKit packages.
- Server Clippy is blocked on Windows by pre-existing unused Unix-side imports in untouched `server/src/files.rs:2–3` (`fs`, `OpenOptions`, `Write`); those platform-specific paths were not deleted to silence the local lint.
- Workspace formatting, documentation checks, frontend typecheck/lint and whitespace checks: passed. Frontend lint reports only existing warnings/informational diagnostics. The release dry run also passed (it validates `origin/main` release metadata, not this branch's native packaging). The standalone server's full rustfmt check still reports pre-existing formatting in untouched `api_tests.rs` and `sessions_tests.rs`; the changed dispatch file is formatted.
- CI/CodSpeed workflow regression run: **102 passed, one failed** in the existing Windows temporary-Git whitespace fixture (`EPERM` during temporary-directory cleanup); the new crate, Docker-context and GUI-free checks passed.

No cluster resources were deployed or modified. Native desktop launch/cluster E2E, macOS/Linux execution, the server's embedded-web-UI variant, Docker image builds and Nix builds remain unverified locally. The embedded variant needs the absent `dist/` frontend build; Docker and Nix executables are unavailable on this host. Raw logs and the Cargo timing report are under `e2e/artifacts/rust-crate-split/`.
