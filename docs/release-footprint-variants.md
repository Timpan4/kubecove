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
