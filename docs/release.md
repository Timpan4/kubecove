# Releases

KubeCove installers publish through [GitHub Releases](https://github.com/Timpan4/kubecove/releases). GitHub Releases is authoritative for published installers; source features are unreleased until matching release tag completes its gates.

## Installer Guide

- macOS: download `.dmg`. Unsigned beta builds may require right-click Open or System Settings approval.
- Windows: download NSIS setup executable. Unsigned beta builds may require SmartScreen's More info then Run anyway.
- Linux: download `.AppImage`, `.deb`, or `.rpm` when present. AppImage may need `chmod +x`.

Installers contain app and normal Tauri runtime files. They do not bundle `kubectl`, Helm, Argo CD, kubeconfigs, tokens, or cluster credentials.

Windows releases use NSIS so in-app updates share one installer path. An older MSI install may require one uninstall before NSIS install.

## Product Safety

KubeCove is inspection-first outside ADR-approved live sessions and guarded operations. Pod and selector-backed Service port-forwarding follows [ADR 0003](decisions/0003-guarded-live-sessions.md); guarded resource operations follow [ADR 0004](decisions/0004-guarded-cluster-operations.md); exact-Pod exec follows [ADR 0005](decisions/0005-guarded-pod-exec-sessions.md); selected-resource YAML apply follows [ADR 0006](decisions/0006-guarded-selected-resource-yaml-apply.md); connected Argo CD follows [ADR 0013](decisions/0013-argocd-connected-inspection-and-operations.md); runtime Secret disclosure follows [ADR 0014](decisions/0014-runtime-secret-disclosure.md).

## Maintainer Release Flow

1. Run GitHub Actions **Prepare Release PR** with `patch`, `minor`, or `major` bump.
2. Review version metadata, generated `CHANGELOG.md` section, `bun run docs:check`, and release claims against implemented guarded-operation contracts.
3. Merge release PR to `main`.

Trusted release PRs skip pull-request CI and CodSpeed. The exemption requires the same-repository `release/app-v*` branch and `release` label. Tag builds still run every automated release gate below before publishing.

After merge, GitHub Actions finds merged `release` PR, creates matching annotated `app-vX.Y.Z` tag, runs release workflow, builds installers, verifies updater assets, and publishes release.

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
