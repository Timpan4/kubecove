# Releases

KubeCove installers publish through [GitHub Releases](https://github.com/Timpan4/kubecove/releases). GitHub Releases is authoritative for published installers. A feature in source is unreleased until the matching release tag completes its gates.

## Installer Guide

- macOS Apple silicon: download the `.dmg`.
- Windows x64: download the NSIS setup executable.
- Linux x64: download the `.AppImage` and make it executable with `chmod +x` if needed.

Installers contain the GPUI desktop app.

The release workflow also builds standalone `kubecove-server` archives for Linux x64 and Linux arm64, each in a full variant with the browser assets embedded and a `-slim` API-only variant, with SHA-256 checksum files. Linux archives include a systemd unit. It also builds full and slim distroless container images for `linux/amd64` and `linux/arm64`, then publishes each version tag as a multi-architecture image beside per-architecture tags. There is no `latest` tag. The server runs without Tauri or webview GUI dependencies and is intended for a single OS user on a remote host, normally reached through SSH forwarding. See [the remote-host operations guide](wiki/Remote-Server.md) and [the install guide](wiki/Server-Installation.md). Server assets are available only after the matching release completes its publish gates.

Windows releases use NSIS so in-app updates share one installer path. An older MSI install may need to be uninstalled once before the NSIS install.

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

Preparation regenerates `bun.nix` from `bun.lock` with the flake's pinned `bun2nix`. Dependabot updates only `bun.lock`, and the daily Nix lock refresh PR may still be open at release time, so without this step a tag can ship a `bun.nix` that breaks every offline Nix build of that tag.

Release PRs use the same change-based frontend, Rust, and CodSpeed checks as other PRs. The Nix build does not run on pull requests. After merge or on manual dispatch, it builds the frontend and desktop app and verifies server dependencies directly from the committed Cargo lockfiles. Tag builds run the release gates for each product described below.

After the merge, GitHub Actions finds the merged `release` PR, creates the matching annotated `app-vX.Y.Z` tag and runs the release workflow. The workflow uploads and verifies the signed GPUI bundles, updater feed, and server archives before pushing container tags to GHCR. GitHub makes the draft release public after the registry push succeeds.

A manual dispatch of the release workflow only reruns an existing `app-v*` tag and preserves the release's visibility.

Validate the `origin/main` release metadata locally:

```sh
bun run release:dry-run
```

`bun run release` does not create or push tags. Releases start from **Prepare Release PR**.

## In-App Updates

In-app updates run only in release-workflow builds. Local development and ad hoc builds use the `dev` channel and do not check GitHub.

Updater artifacts require the GitHub Actions secret `TAURI_SIGNING_PRIVATE_KEY`. Its password is optional. The GPUI client compiles in the public key and verifies the minisign signatures.

Release notes mirror the matching [CHANGELOG.md](../CHANGELOG.md) section.

### Native GPUI client

The desktop release ships `KubeCove` under the identifier `com.timpan.kubecove`. `.github/workflows/gpui-release.yml` builds it with `cargo build --locked` and bundles it with `cargo packager`, configured in `[package.metadata.packager]` of `rust/gpui/Cargo.toml`:

| Platform | Release asset |
| --- | --- |
| Linux x64 | `kubecove_<version>_x86_64.AppImage` |
| macOS Apple silicon | `kubecove_<version>_aarch64.dmg` to install, and `kubecove_<version>_aarch64.app.tar.gz`, with `KubeCove.app` at the archive root, for the updater. There is no Intel macOS build. |
| Windows x64 | `kubecove_<version>_x64-setup.exe`, a per-user NSIS installer |

The DMG is the first-install download. The updater uses the signed app archive. One Linux job signs each updater asset with standalone `minisign` and the existing `TAURI_SIGNING_PRIVATE_KEY`. Then `scripts/gpui-release-manifest.mjs` checks every `.sig` against the public key in `rust/gpui/updater.json` and checks that its trusted comment names the exact file and version. It writes `latest-desktop.json` with `linux-x86_64`, `darwin-aarch64` and `windows-x86_64` entries. The `stage-release-assets` job uploads the assets, their signatures and `latest-desktop.json` to the release, and carries forward the `latest.json` and `latest-gpui.json` manifests of the `0.15.2` pitstop untouched for legacy installs.

The NSIS installer accepts `/P` for a passive install, `/R` to start the app after a silent or passive install, and `/NS` to skip shortcuts. In passive mode it closes a running `kubecove-gpui.exe` without asking.

The GPUI crate version must equal the app version, because the client compares it with the manifest. `bun scripts/bump-version.ts` and release preparation update `rust/gpui/Cargo.toml` and `rust/gpui/Cargo.lock`, and release validation rejects a mismatch.

To prove the GPUI build, bundle and signing steps on a branch without touching any release:

```sh
gh workflow run release.yml --ref <branch> -f gpui-dry-run=true
```

Every other release job skips a dry run, so nothing reads or writes a release or tag, and the manifest URLs point at `app-v<version>`. The signed bundles and `latest-desktop.json` arrive as the `kubecove-gpui-release` workflow artifact.

## Automated Release Gates

Release tags run frontend checks, server packaging, and GPUI bundle and signature verification before publication. Native Nix builds for `x86_64-linux` and `aarch64-linux` run in parallel as advisory checks. Nix users run the GPUI client from the tagged source flake.

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
