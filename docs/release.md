# Releases

KubeCove installers publish through [GitHub Releases](https://github.com/Timpan4/kubecove/releases). GitHub Releases is authoritative for published installers. A feature in source is unreleased until the matching release tag completes its gates.

## Installer Guide

- macOS: download the `.dmg`. Unsigned beta builds may require right-click Open or System Settings approval.
- Windows: download the NSIS setup executable. Unsigned beta builds may require SmartScreen's More info, then Run anyway.
- Linux: download the `.AppImage`, `.deb`, or `.rpm` when present. The AppImage may need `chmod +x`.

Installers contain the desktop app and normal Tauri runtime files. They do not bundle `kubectl`, Helm, Argo CD, kubeconfigs, tokens, or cluster credentials.

The release workflow also builds standalone `kubecove-server` archives for Linux x64 and Linux arm64, each in a full variant with the browser assets embedded and a `-slim` API-only variant, with SHA-256 checksum files. Linux archives include a systemd unit. It also builds full and slim distroless container images for `linux/amd64` and `linux/arm64`, then publishes each version tag as a multi-architecture image beside per-architecture tags. There is no `latest` tag. The server runs without Tauri or webview GUI dependencies and is intended for a single OS user on a remote host, normally reached through SSH forwarding. See [the remote-host operations guide](wiki/Remote-Server.md) and [the install guide](wiki/Server-Installation.md). Server assets are available only after the matching release completes its publish gates.

Windows releases use NSIS so in-app updates share one installer path. An older MSI install may need to be uninstalled once before the NSIS install.

### Linux AppImage WebKitGTK

The AppImage bundles the WebKitGTK stack from its Ubuntu build runner. At launch, the binary uses the host stack instead when the host `libwebkit2gtk-4.1.so.0` is newer:

1. It finds the host `libwebkit2gtk-4.1.so.0`, `libgtk-3.so.0`, and `libglib-2.0.so.0` through `ldconfig -p`, matching the soname and the 64-bit ABI flag. It continues only when all three are in one directory, so host WebKit never resolves GTK or GLib from the AppImage.
2. It reads the bundled version from the loaded library with `webkit_get_major_version`, `webkit_get_minor_version`, and `webkit_get_micro_version`.
3. It starts itself once as a probe with the host library directory first in `LD_LIBRARY_PATH` and `LD_BIND_NOW=1`, and the probe prints the same three values. Every library must resolve its symbols against that stack at load, so an incompatible host stack fails the probe. The probe does not initialize GTK or start a web process, so a host WebKit that fails at runtime is not detected.
4. If the probe succeeds with a newer version, the binary logs the switch to stderr and re-executes itself with that `LD_LIBRARY_PATH`. Host libraries in that directory then win; libraries the host keeps elsewhere can still resolve to bundled copies. The re-exec drops the GTK hook's variables that point into the AppImage (`GTK_PATH`, `GIO_MODULE_DIR`, `GSETTINGS_SCHEMA_DIR`, `GDK_PIXBUF_MODULE_FILE`, and related) and the AppImage entries of `XDG_DATA_DIRS`, `GST_PLUGIN_SYSTEM_PATH`, and `GST_PLUGIN_SYSTEM_PATH_1_0`, so host GTK, GLib, and GStreamer load host modules, schemas, and plugins. Otherwise the launch continues on the bundled stack; when a host stack was found but the probe failed, it logs that to stderr.

Set `KUBECOVE_APPIMAGE_WEBKIT=bundled` to keep the bundled stack. `system` and `probe` are internal re-exec values; other values are ignored with a warning. Processes the app starts inherit the host-first `LD_LIBRARY_PATH`. Tauri's AppImage bundler copies `bundle.linux.appimage.files` only under `usr/`, so it cannot add an AppRun hook. The switch therefore lives in the binary, and the signed AppImage file stays as the Tauri bundler produced it. Other Linux packages and AppDirs without `APPDIR` skip the check.

## Product Safety

KubeCove is inspection-first outside ADR-approved live sessions and guarded operations. Pod and selector-backed Service port-forwarding follows [ADR 0003](decisions/0003-guarded-live-sessions.md); guarded resource operations follow [ADR 0004](decisions/0004-guarded-cluster-operations.md); exact-Pod exec follows [ADR 0005](decisions/0005-guarded-pod-exec-sessions.md); selected-resource YAML apply follows [ADR 0006](decisions/0006-guarded-selected-resource-yaml-apply.md); connected Argo CD follows [ADR 0013](decisions/0013-argocd-connected-inspection-and-operations.md); runtime Secret disclosure follows [ADR 0014](decisions/0014-runtime-secret-disclosure.md).

## Maintainer Release Flow

1. Run the GitHub Actions workflow **Prepare Release PR** with a `patch`, `minor`, or `major` bump.
2. Review the version metadata, the generated `CHANGELOG.md` section and `bun run docs:check`, and check release claims against the implemented guarded-operation contracts.
3. Merge the release PR to `main`.

Preparation commits the generated files through GitHub's signed-commit API and verifies the signature before opening the PR. Reruns append to an existing release branch without rewriting its history.

Release preparation uses the existing `NIX_LOCK_REFRESH_TOKEN` secret for both signed API commits and PR creation. This lets PR creation and later branch updates trigger eligible `opened` and `synchronize` checks. The token must have repository Contents and Pull requests write access, including permission to apply the `release` label. Preparation checks token availability and repository access before making changes. Write permissions and commit signing are verified by the subsequent API operations. Failures stop preparation; it never falls back to `GITHUB_TOKEN` or unsigned commits.

The required `Typecheck and test` and `CodSpeed Performance Analysis` checks must come from an eligible event, such as `pull_request` or `push`. Jobs started with `workflow_dispatch` do not satisfy PR ruleset checks, even when they succeed on the same head commit. See [GitHub's required-check rules](https://docs.github.com/en/pull-requests/how-tos/merge-and-close-pull-requests/troubleshooting-required-status-checks).

GitHub puts pull-request workflows created by `GITHUB_TOKEN` into an approval-required state. A maintainer with write access can approve the workflows from the PR page. PR creation with a GitHub App installation token or a personal access token starts eligible PR workflows without that hold. See [GitHub's workflow trigger rules](https://docs.github.com/en/actions/how-tos/write-workflows/choose-when-workflows-run/trigger-a-workflow).

If signature verification fails after commit creation, the branch may already contain that commit. Preparation preserves the branch and stops. Reruns check every commit introduced since the checked-out `main` and reject unverified history before appending. Verify the rejected signature or prepare a new signed release branch before rerunning.

Trusted release PRs skip frontend, Rust, and CodSpeed checks. The exemption requires the same-repository `release/app-v*` branch and `release` label. The Nix build does not run on pull requests. After merge or on manual dispatch, it builds the frontend and desktop app and verifies server dependencies directly from the committed Cargo lockfiles. Tag builds still run every automated release gate below before publishing.

After the merge, GitHub Actions finds the merged `release` PR, creates the matching annotated `app-vX.Y.Z` tag and runs the release workflow. The workflow builds the desktop installers, server archives and container images, verifies the required assets and updater coverage, then publishes the release.

A manual dispatch of the release workflow only reruns an existing `app-v*` tag and preserves the release's visibility.

Validate the `origin/main` release metadata locally:

```sh
bun run release:dry-run
```

`bun run release` does not create or push tags. Releases start from **Prepare Release PR**.

## In-App Updates

In-app updates run only in release-workflow builds. Local development and ad hoc builds use the `dev` channel and do not check GitHub.

Updater artifacts require the GitHub Actions secret `TAURI_SIGNING_PRIVATE_KEY`. Its password is optional. The public key is committed in `src-tauri/tauri.conf.json`. Installer packages may remain unsigned beta builds, but updater artifacts and `latest.json` must be signed.

Release notes mirror the matching [CHANGELOG.md](../CHANGELOG.md) section.

### Native GPUI client

The release also ships the native GPUI client (`src-tauri/gpui`) as `KubeCove GPUI`, which installs beside the Tauri app under its own identifier, `com.timpan.kubecove.gpui`. `.github/workflows/gpui-release.yml` builds it with `cargo build --locked` and bundles it with `cargo packager`, configured in `[package.metadata.packager]` of `src-tauri/gpui/Cargo.toml`:

| Platform | Release asset |
| --- | --- |
| Linux x64 | `kubecove-gpui_<version>_x86_64.AppImage` |
| macOS Apple silicon | `kubecove-gpui_<version>_aarch64.dmg` to install, and `kubecove-gpui_<version>_aarch64.app.tar.gz`, with `KubeCove GPUI.app` at the archive root, for the updater. There is no Intel macOS build. |
| Windows x64 | `kubecove-gpui_<version>_x64-setup.exe`, a per-user NSIS installer |

The DMG is the first-install download. Like the Tauri DMG, it is not in the updater feed and gets no `.sig`. One Linux job signs each updater asset with `tauri signer sign --app-version <version>` and the same `TAURI_SIGNING_PRIVATE_KEY`, then `scripts/gpui-release-manifest.mjs` checks every `.sig` against the public key in `src-tauri/tauri.conf.json` and checks that its trusted comment names `version:<version>`. It writes `latest-gpui.json` in Tauri's static updater format, with `linux-x86_64`, `darwin-aarch64` and `windows-x86_64` entries. The publish job uploads the assets, their signatures and `latest-gpui.json` to the release. `latest.json` stays the Tauri app's feed.

The NSIS installer accepts `/P` for a passive install, `/R` to start the app after a silent or passive install, and `/NS` to skip shortcuts. In passive mode it closes a running `kubecove-gpui.exe` without asking.

The GPUI crate version must equal the app version, because the client compares it with the manifest. `bun scripts/bump-version.ts` and release preparation update `src-tauri/gpui/Cargo.toml` and `src-tauri/gpui/Cargo.lock`, and release validation rejects a mismatch.

To prove the GPUI build, bundle and signing steps on a branch without touching any release:

```sh
gh workflow run release.yml --ref <branch> -f gpui-dry-run=true
```

Every other release job skips a dry run, so nothing reads or writes a release or tag, and the manifest URLs point at `app-v<version>`. The signed bundles and `latest-gpui.json` arrive as the `kubecove-gpui-release` workflow artifact.

## Automated Release Gates

Release tags run type checks, unit tests, Rust tests and checks, deterministic fast E2E, and real E2E across the supported Kubernetes matrix before installers build. Native Nix builds for `x86_64-linux` and `aarch64-linux` run in parallel as advisory checks, use the GitHub Actions Nix cache, and do not delay or block publishing. The workflow verifies installer assets, updater signatures, `latest.json`, and updater platform coverage before publishing. Nix users run the tagged source flake; releases do not attach a separate Nix archive.

Support window and matrix changes follow [ADR 0011](decisions/0011-rolling-kubernetes-support.md). Real E2E uses [ADR 0012](decisions/0012-production-shaped-e2e-lab.md).

## Manual Smoke Test

1. Install and start the target release artifact.
2. Confirm that the context list never exposes raw kubeconfig contents.
3. Open a readable workspace and confirm namespace and resource browsing.
4. Confirm selected-resource details, YAML, events, and logs when available.
5. Confirm that guarded exec shows the exact Pod target and command, requires confirmation, streams output, accepts stdin, resizes, and stops.
6. Confirm that guarded operations show exact targets, previews or preflight where required, and a final confirmation.
7. Check the Argo CD and Helm sections when matching cluster metadata exists.
8. Confirm that per-key Secret reveal is transient and that connected Argo Secret payloads remain redacted.

## Publishing Checklist

- Confirm that the release PR version, changelog, docs check, and release notes are accurate.
- Confirm that every claimed operation has a typed command, the required preview or preflight, confirmation UX, and ADR coverage.
- After publishing, optionally smoke-test the installer against context listing, resource browsing, and unavailable-cluster errors.

For a bad release, run **Yank Release** on the bad `app-v*` tag. `draft` hides the release and keeps its artifacts. `delete-release` removes the GitHub Release. Ship the patch through the normal release PR flow.
