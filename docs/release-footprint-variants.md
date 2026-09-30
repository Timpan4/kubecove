# Release footprint characterization

Issue [#421](https://github.com/Timpan4/kubecove/issues/421) compares stable build settings without changing the shipped configuration. Policy selection belongs to #422.

The report must use one clean source SHA, frozen frontend output, package format, and startup path for every variant. Production package sizes and companion release-shaped E2E measurements must remain distinct because production builds exclude WebDriver. No capability may be removed to make unused-command removal look smaller.

Failure modes to verify before implementing the reporter:

- Frontend assets embedded in the executable are added again to package totals.
- Compressed package overhead is compared with uncompressed installed bytes.
- Bundled fonts are mistaken for requested fonts.
- A changed source, frontend hash, dependency lockfile, feature set, or startup path appears as a like-for-like comparison.
- A failed build, benchmark, startup, or crash probe appears as zero or a success.
- Symbol stripping removes diagnostic names but that cost is omitted.
- Panic-abort benchmark results are labeled abort even though Cargo's test harness forces unwind.
- Unused-command removal breaks typed commands or required plugin permissions.
- The kubeconform sidecar is dropped from a package-size comparison.
- An owned build target or extracted package is confused with a user installation.

The axes come from the issue: baseline; thin and fat LTO; one codegen unit; symbol stripping; speed mode `2`; size modes `s` and `z`; panic abort; Tauri unused-command removal. Each changes one setting from the baseline. This is a characterization matrix, not a recommended combined profile.

Cargo's [stable profile reference](https://doc.rust-lang.org/cargo/reference/profiles.html) defines these settings and states that tests and benchmarks ignore panic strategy. Tauri's [configuration reference](https://v2.tauri.app/reference/config/#removeunusedcommands) describes unused-command removal against the allowed plugin command list. It does not authorize narrowing capabilities or claim that app commands disappear.

## Reproduction

Run `bun run perf:release` from a clean checkout with frozen dependencies, Cargo on PATH, a working Linux display, and the existing native E2E dependencies and session keyring. The current package probe uses Linux ELF, Debian packages, `readelf`, `ar`, `dpkg-deb`, `prlimit`, and OpenSSL. It extracts an owned package without installing it. Other platforms require their package-specific probes; this command does not change the cross-platform application.

Reports, logs, screenshots, and extracted packages stay under `e2e/artifacts/release-footprint-*/`. Isolated compiler targets and frozen frontend directories stay under `.e2e/release-footprint/`. Each variant starts with an empty target directory. Production build time includes dependency compilation and Debian bundling, while Cargo's downloaded source cache and the host filesystem cache remain warm. Runs execute sequentially, and do not stop an existing app or development server.

The baseline explicitly uses Cargo's stable release defaults: optimization `3`, local ThinLTO (`lto = false`), 16 codegen units, no debug information, no symbol stripping, and panic unwinding. Production packages use one frozen normal frontend. Native startup companions use a separate frozen frontend with the existing profile flag and E2E feature. All variants share each corresponding frontend hash, source SHA, fixture, permissions, and scenario. Updater signing is disabled only for these local characterization packages.

The native scenario opens a fresh launcher, creates a workspace against 65 synthetic Pods, opens Resources and YAML, and records existing startup marks and process groups. It uses DOM events and includes driver overhead. Fonts are identified by loaded CSS FontFace objects matched to bundled font URLs. WebKit exposes no resource timing entries for this embedded protocol here; these are font-loading observations, not network timing. No fonts are fetched from an external service. Font files bundled but unloaded stay distinct from files loaded at each checkpoint.

Frontend raw asset bytes are a composition report. They are already embedded in the executable and must not be added to installed or package totals. Rust library outputs are separately reported compiler artifacts, not extra shipped package files. ELF removable symbol tables and debug sections are a subset of executable bytes; runtime dynamic tables remain separate. ELF section bytes omit NOBITS sections and do not include file headers or padding. Compressed archive members plus container overhead equal package bytes. Standalone gzip size estimates the sidecar's compression potential; it is not an additive attribution inside the package's compressed data archive.

The production crash probe triggers the existing E2E-feature guard before app initialization, with backtraces enabled and core dumps disabled only for that child. It records the exit or signal and availability of named frames. It does not represent every production panic or minidump path. Existing backend hot-path benchmarks run with the same optimization settings; Cargo forces benchmark panic behavior to unwind, including in the panic-abort row.

Unused-command removal retains the production capability file. Its companion adds the existing E2E capability alongside that file. The probe checks generated allowed-command lists for opener, process restart, and the full updater workflow, and confirms application commands have no app ACL that would prune them. Native Resources and YAML exercise typed read paths. Invalid resource arguments test updater dispatch without downloading or installing. Process restart, opening an external browser, and actual updates are not executed; their unchanged permission and registration paths are checked at build time.

## Measured Linux cohort

The matrix completed on September 30, 2026 from clean source `7d98fb9435948beb46dbf3f75f396c0dd13c7bb5`. All ten production packages, nine existing benchmark families per variant, and native launcher/Resources/YAML scenarios passed. Each native run ended with its owned fixture removed. Baseline, stripped, and unused-command-removal YAML screenshots were visually inspected.

The host used Debian 13.7, Linux `6.12.96+deb13-amd64`, x64, 16 logical CPUs, Rust 1.98.1, Bun 1.4.2, GTK 3.24.49, and WebKitGTK 2.52.6. Weston 14.0.2 ran headless with software rendering. Every native checkpoint had an 800 by 553 CSS-pixel viewport. These measurements characterize this host, not Windows, macOS, hardware-accelerated rendering, or a distribution of startup times. Each build/startup observation is one run; no runtime improvement is established by a small difference between rows.

The local evidence directory is `e2e/artifacts/release-footprint-1790723583779-b0d75851-ef8e-4598-b26d-ea1978c28096/`. It contains `release-footprint.json`, per-variant production and companion build logs, `benchmarks.txt`, `crash.txt`, `wdio-report.txt`, `footprint-startup.json`, and `yaml.png`. Artifacts remain ignored rather than shipping extracted packages or machine-specific logs in Git.

After collection, review corrected unobserved asset-summary font-request fields from empty arrays/zero to `null`. The original bytes remain in `release-footprint.raw.json`, SHA-256 `2a6815f3a64ba59882d20764a41142d873c980c7e9ff972b03a7f848a2d16693`. `reportingCorrection` identifies the four availability fields. No package, timing, memory, benchmark, crash, or native font observation changed. The reporter now emits the correct availability directly. A separate test-first correction makes SIGINT/SIGTERM reap the owned detached driver and remove fixture files. These tooling corrections do not change the measured application, frontend, profile settings, or cohort provenance.

### Asset and package composition

The normal production frontend fingerprint is `a31f1426d4bc5e776f5e1c6a0b648192ff10c02473bd2170963f90846a0eaeac`. Its 2,681,648 raw bytes consist of 2,131,529 JavaScript, 171,559 CSS, 365,380 fonts across 18 files, and 13,180 other assets. These are embedded composition bytes, not an additional installed payload.

All variants retain kubeconform v0.7.0: 13,562,792 raw bytes and 7,499,991 bytes under standalone gzip. The latter is a compression estimate, not its share of `data.tar.gz`. The baseline package contains a 4-byte `debian-binary`, 518-byte `control.tar.gz`, 23,691,851-byte `data.tar.gz`, and 189 bytes of archive container overhead, totaling 23,692,562 bytes. Installed bytes comprise the 55,001,760-byte executable, sidecar, and 58,246 bytes of other files. The baseline executable includes 22,005,012 bytes of removable ELF symbols and 31,206 bytes of runtime symbol tables; neither is additive to executable size.

At both launcher and workspace checkpoints, every variant loaded `geist-latin-wght-normal-BgDaEnEv.woff2` (29,400 bytes) and `inter-latin-wght-normal-Dx4kXJAl.woff2` (48,256 bytes). YAML loaded `geist-mono-latin-wght-normal-XN7g48iV.woff2` (23,128 bytes) and Inter Latin. The remaining bundled files stay visible in the inventory. Loaded-face observations identify local files used at those checkpoints; they are not a request timeline or cumulative download history. Asset-only summaries mark requests unavailable.

### Size, build, and crash observations

MiB uses 1,048,576 bytes. Build time covers cold isolated dependency compilation and Debian packaging with shared downloaded sources and host caches. Sizes below measure normal production packages; startup and memory below measure their E2E companions.

| Variant | Debian MiB | Executable MiB | Cold build seconds | Removable symbols MiB | Crash exit | Named app frame |
| --- | --- | --- | --- | --- | --- | --- |
| baseline | 22.59 | 52.45 | 85.73 | 20.99 | 101 | present |
| thin-lto | 22.29 | 50.01 | 107.76 | 18.12 | 101 | present |
| fat-lto | 20.55 | 36.71 | 238.26 | 10.19 | 101 | present |
| one-codegen-unit | 20.47 | 38.93 | 188.53 | 13.44 | 101 | present |
| strip-symbols | 19.31 | 31.50 | 85.12 | 0.00 | 101 | absent |
| opt-level-2 | 22.39 | 52.87 | 84.29 | 20.66 | 101 | present |
| opt-level-s | 20.88 | 52.29 | 74.07 | 25.10 | 101 | present |
| opt-level-z | 21.20 | 57.86 | 69.09 | 31.23 | 101 | present |
| panic-abort | 19.48 | 41.26 | 79.32 | 16.80 | SIGABRT | present |
| remove-unused-commands | 22.20 | 49.60 | 80.66 | 19.53 | 101 | present |

### Native startup and process memory

| Variant | Launcher ms | First rows ms | Launcher host/WebView MiB | Workspace host/WebView MiB | YAML host/WebView MiB |
| --- | --- | --- | --- | --- | --- |
| baseline | 117 | 2255 | 220.72 / 479.11 | 227.85 / 632.13 | 230.68 / 747.64 |
| thin-lto | 152 | 2251 | 221.42 / 477.11 | 227.22 / 629.93 | 229.75 / 748.67 |
| fat-lto | 120 | 2197 | 216.13 / 480.32 | 223.31 / 628.13 | 225.95 / 729.20 |
| one-codegen-unit | 118 | 2239 | 217.16 / 479.16 | 222.91 / 628.45 | 225.50 / 732.17 |
| strip-symbols | 126 | 2209 | 220.88 / 464.25 | 227.25 / 620.50 | 229.22 / 722.26 |
| opt-level-2 | 120 | 2274 | 221.84 / 479.31 | 228.13 / 628.61 | 231.16 / 724.29 |
| opt-level-s | 120 | 2258 | 220.13 / 482.81 | 226.39 / 630.89 | 228.92 / 759.42 |
| opt-level-z | 115 | 2268 | 219.17 / 479.52 | 224.73 / 620.47 | 226.91 / 751.95 |
| panic-abort | 152 | 2056 | 204.49 / 469.32 | 209.18 / 618.83 | 211.66 / 722.76 |
| remove-unused-commands | 115 | 2215 | 214.67 / 472.04 | 221.21 / 612.98 | 223.84 / 726.53 |

### Existing backend hot paths

| Benchmark median | baseline | thin-lto | fat-lto | one-codegen-unit | strip-symbols | opt-level-2 | opt-level-s | opt-level-z | panic-abort | remove-unused-commands |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| build_ownership_topology_500_apps | 3.39 ms | 3.082 ms | 3.282 ms | 3.405 ms | 3.654 ms | 3.57 ms | 4.118 ms | 5.066 ms | 3.74 ms | 3.634 ms |
| build_ownership_topology_500_apps_with_crd_overlay | 5.578 ms | 5.035 ms | 5.075 ms | 4.864 ms | 5.486 ms | 5.467 ms | 6.362 ms | 7.469 ms | 5.683 ms | 5.343 ms |
| helm_manifest_parse_250_resources | 1.025 ms | 1.065 ms | 998.4 µs | 1.114 ms | 1.062 ms | 1.068 ms | 1.595 ms | 2.04 ms | 1.077 ms | 1.036 ms |
| helm_manifest_summary_250_resources | 3.556 µs | 3.145 µs | 3.065 µs | 3.435 µs | 3.635 µs | 3.716 µs | 3.776 µs | 3.862 µs | 3.606 µs | 3.566 µs |
| inspect_cached_payload_weights_500_apps | 129.8 ns | 120.8 ns | 129.8 ns | 129.8 ns | 180.8 ns | 139.8 ns | 219.8 ns | 330.8 ns | 139.8 ns | 111.5 ns |
| present_custom_resource_scope_key_250_namespaces | 9.958 µs | 9.167 µs | 9.101 µs | 9.958 µs | 10.05 µs | 10.1 µs | 10.62 µs | 11.98 µs | 10.33 µs | 10.09 µs |
| serialize_large_resource_kyaml | 116.9 µs | 112 µs | 107.3 µs | 110.6 µs | 119.4 µs | 121.2 µs | 136.2 µs | 147.8 µs | 118.7 µs | 120.2 µs |
| serialize_large_resource_yaml | 99.95 µs | 98.36 µs | 93 µs | 100.8 µs | 97.45 µs | 101.4 µs | 209.3 µs | 266.5 µs | 100.4 µs | 97.13 µs |
| sort_custom_resource_catalog_1000_kinds | 297.8 µs | 289.7 µs | 282.3 µs | 291.2 µs | 295 µs | 295.9 µs | 316.7 µs | 565.8 µs | 296 µs | 298.4 µs |

### Interpretation for the owner decision

No production setting is selected here. Keeping the existing configuration remains valid under [#422](https://github.com/Timpan4/kubecove/issues/422).

- Symbol stripping made the smallest package in this cohort, but removed the named application frame from the crash probe.
- Panic abort also reduced package and executable size. Its probe retained named frames with `RUST_BACKTRACE=1` and terminated through `SIGABRT`; it changes unwinding and cleanup semantics. The benchmark harness still used unwinding.
- Fat LTO and one codegen unit reduced size while retaining named crash frames, with longer cold builds. Their relative hot-path medians differ by workload.
- Size optimization `s` and `z` reduced the compressed package but slowed topology, parsing, and YAML workloads in these samples. `z` increased the unstripped executable because its removable symbol tables grew.
- Unused-command removal reduced package size with unchanged production capabilities. Required opener, restart, updater, and application registrations remained present; native typed reads and invalid-argument updater dispatch passed. Real restart, external-browser opening, and update installation remain outside this safe probe.

Launcher and first-row marks include the automated interaction path. Process RSS includes mapped/shared pages and driver/software-display effects; it does not attribute retained JavaScript objects or prove a leak. Native WebKit exposes no JavaScript heap figure here. The table does not establish a causal startup or memory improvement. Backend benchmark medians use Divan's existing default 100 samples, and CodSpeed remains the primary deterministic performance signal.

The owner must choose the priority among package size, build time, runtime behavior, memory, and crash diagnostics before any production change. Combined settings were not measured and cannot inherit the isolated rows' results. A selected production configuration still requires clean reproduction and the Windows, macOS universal, and Linux builds and smoke checks required by #422.

Verification passed: six focused report/interrupt tests, explicit script and test TypeScript checks, frontend typecheck, Biome lint, anti-slop lint, docs checks, 330 Rust tests with one existing ignored test, Cargo check, and the release dry run. Biome's existing `App.css` warnings remain outside this change. The final signal-aware helper also passed a normal native run using the unchanged command-removal companion; evidence is under `e2e/artifacts/footprint-cleanup-smoke-1790727097535/`, separate from the comparison cohort.
