# Local updater dependency patch

This directory contains the build inputs and licenses from the published `tauri-plugin-updater` 2.13.3 crate. The only upstream file changed is `Cargo.toml`: the Windows ZIP dependency changes from `4` to `=9.0.5`, retaining `optional = true` and `default-features = false`.

Source: https://crates.io/crates/tauri-plugin-updater/2.13.3

Published crate SHA-256: `855a336eb389d2c1502244fd48fa2d8cb487a430f691bbaed025195f99a04489`.

Upstream repository: https://github.com/tauri-apps/plugins-workspace/tree/6bbd44d4ef254d835a12c6d7bdf2a8a61778031a/plugins/updater

The published updater requires ZIP 4. Its latest ZIP 4 release, 4.6.1, is affected by RUSTSEC-2026-0367, RUSTSEC-2026-0368, and RUSTSEC-2026-0369. ZIP 9.0.5 contains the fixes and supports the updater's existing archive construction, extraction, and error conversion calls. Its Rust 1.88 minimum is below this project's Rust 1.91 minimum.

The Rust source, permissions, build script, and JavaScript API are unchanged. The large upstream `src/updater.rs` file is exempt from the project file-size hook so this dependency patch does not restructure third-party code. The source package's standalone lockfile, original workspace manifest, changelog, registry metadata, and security-reporting guide are omitted because Cargo does not need them to build this path dependency. The desktop workspace's lockfile owns dependency resolution.

Remove this directory, the root `[patch.crates-io]` entry, its workspace exclusion, and the corresponding file-size exception when an upstream updater release accepts a ZIP version that passes the dependency audit. Keep the updater feature and signed-update behavior intact during that replacement.
