# Releases

KubeCove installers publish through [GitHub Releases](https://github.com/Timpan4/kubecove/releases). GitHub Releases is authoritative for published installers; source features are unreleased until matching release tag completes its gates.

## Installer Guide

- macOS: download `.dmg`. Unsigned beta builds may require right-click Open or System Settings approval.
- Windows: download NSIS setup executable. Unsigned beta builds may require SmartScreen's More info then Run anyway.
- Linux: download `.AppImage`, `.deb`, or `.rpm` when present. AppImage may need `chmod +x`.

Installers contain the desktop app and normal Tauri runtime files. They do not bundle `kubectl`, Helm, Argo CD, kubeconfigs, tokens, or cluster credentials.

The release workflow also builds standalone `kubecove-server` archives for Linux x64, Linux arm64 and macOS arm64, each in a full variant with the browser assets embedded and a `-slim` API-only variant, with SHA-256 checksum files. Linux archives include a systemd unit. It also builds full and slim distroless container images for `linux/amd64` and `linux/arm64`, then publishes each tag as a multi-architecture image. The server runs without Tauri or webview GUI dependencies and is intended for a single OS user on a remote host, normally reached through SSH forwarding. See [the remote-host operations guide](wiki/Remote-Server.md) and [the install guide](wiki/Server-Installation.md). Server assets are available only after the matching release completes its publish gates.

Windows releases use NSIS so in-app updates share one installer path. An older MSI install may require one uninstall before NSIS install.

### Linux AppImage WebKitGTK

The AppImage bundles the WebKitGTK stack from its Ubuntu build runner. At launch, the binary uses the host stack instead when the host `libwebkit2gtk-4.1.so.0` is newer:

1. It finds the host library through `ldconfig -p`. It matches the soname and the 64-bit ABI flag, not a fixed directory.
2. It reads the bundled version from the loaded library with `webkit_get_major_version`, `webkit_get_minor_version`, and `webkit_get_micro_version`.
3. It starts itself once as a probe with the host library directory first in `LD_LIBRARY_PATH`, and the probe prints the same three values. The binary links with `BIND_NOW`, so a host stack that cannot satisfy it makes the probe fail.
4. If the probe succeeds with a newer version, the binary re-executes itself with that `LD_LIBRARY_PATH`. Host libraries then win, and bundled libraries fill only the gaps. Otherwise the launch continues on the bundled stack.

Set `KUBECOVE_APPIMAGE_WEBKIT=bundled` to keep the bundled stack. Tauri's AppImage bundler copies `bundle.linux.appimage.files` only under `usr/`, so it cannot add an AppRun hook. The switch therefore lives in the binary, and the signed AppImage file stays as the Tauri bundler produced it. Other Linux packages and AppDirs without `APPDIR` skip the check.

## Product Safety

KubeCove is inspection-first outside ADR-approved live sessions and guarded operations. Pod and selector-backed Service port-forwarding follows [ADR 0003](decisions/0003-guarded-live-sessions.md); guarded resource operations follow [ADR 0004](decisions/0004-guarded-cluster-operations.md); exact-Pod exec follows [ADR 0005](decisions/0005-guarded-pod-exec-sessions.md); selected-resource YAML apply follows [ADR 0006](decisions/0006-guarded-selected-resource-yaml-apply.md); connected Argo CD follows [ADR 0013](decisions/0013-argocd-connected-inspection-and-operations.md); runtime Secret disclosure follows [ADR 0014](decisions/0014-runtime-secret-disclosure.md).

## Maintainer Release Flow

1. Run GitHub Actions **Prepare Release PR** with `patch`, `minor`, or `major` bump.
2. Review version metadata, generated `CHANGELOG.md` section, `bun run docs:check`, and release claims against implemented guarded-operation contracts.
3. Merge release PR to `main`.

Preparation commits the generated files through GitHub's signed-commit API and verifies the signature before opening the PR. Reruns append to an existing release branch without rewriting its history.

If signature verification fails after commit creation, the branch may already contain that commit. Preparation preserves the branch and stops. Reruns check every commit introduced since the checked-out `main` and reject unverified history before appending. Verify the rejected signature or prepare a new signed release branch before rerunning.

Trusted release PRs skip frontend, Rust, and CodSpeed checks. The exemption requires the same-repository `release/app-v*` branch and `release` label. The Nix check builds the frontend and desktop app and verifies server dependencies directly from the committed Cargo lockfiles. Tag builds still run every automated release gate below before publishing.

After merge, GitHub Actions finds merged `release` PR, creates matching annotated `app-vX.Y.Z` tag, runs release workflow, builds desktop installers and headless server archives, verifies required assets and updater coverage, then publishes the release.

Manual release-workflow dispatch only reruns an existing `app-v*` tag and preserves release visibility.

Validate `origin/main` release metadata locally:

```sh
bun run release:dry-run
```

`bun run release` does not create or push tags. Releases start from **Prepare Release PR**.

## In-App Updates

In-app updates run only in release-workflow builds. Local development and ad hoc builds use `dev` channel and do not check GitHub.

Updater artifacts require GitHub Actions `TAURI_SIGNING_PRIVATE_KEY`; password is optional. Public key is committed in `src-tauri/tauri.conf.json`. Installer packages may remain unsigned beta builds, but updater artifacts and `latest.json` must be signed.

Release notes mirror matching [CHANGELOG.md](../CHANGELOG.md) section.

## Automated Release Gates

Release tags run type checks, unit tests, Rust tests and checks, deterministic fast E2E, and real E2E across the supported Kubernetes matrix before installers build. Native Nix builds for `x86_64-linux` and `aarch64-linux` run in parallel as advisory checks, use the GitHub Actions Nix cache, and do not delay or block publishing. Workflow verifies installer assets, updater signatures, `latest.json`, and updater platform coverage before publishing. Nix users run the tagged source flake; releases do not attach a separate Nix archive.

Support window and matrix changes follow [ADR 0011](decisions/0011-rolling-kubernetes-support.md). Real E2E uses [ADR 0012](decisions/0012-production-shaped-e2e-lab.md).

## Manual Smoke Test

1. Install and start target release artifact.
2. Confirm context listing never exposes raw kubeconfig contents.
3. Open readable workspace; confirm namespace and resource browsing.
4. Confirm selected-resource details, YAML, events, and logs when available.
5. Confirm guarded exec shows exact Pod target and command, requires confirmation, streams output, accepts stdin, resizes, and stops.
6. Confirm guarded operations show exact targets, previews or preflight where required, and final confirmation.
7. Check Argo CD and Helm sections when matching cluster metadata exists.
8. Confirm per-key Secret reveal is transient; connected Argo Secret payloads remain redacted.

## Publishing Checklist

- Confirm release PR version, changelog, docs check, and release notes are accurate.
- Confirm every claimed operation has typed command, required preview or preflight, confirmation UX, and ADR coverage.
- After publish, optionally smoke installer against context listing, resource browsing, and unavailable-cluster errors.

For a bad release, use **Yank Release** for bad `app-v*` tag: `draft` hides release while retaining artifacts; `delete-release` removes GitHub Release. Ship patch through normal release PR flow.
