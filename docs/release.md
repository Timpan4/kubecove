# Beta Releases

KubeCove beta installers are published as GitHub Releases:

https://github.com/Timpan4/kubecove/releases

Use a release installer when testing the app. Build from source only for development work.

## Installer Guide

- macOS: download the `.dmg`. The beta is unsigned; macOS may require right-click Open or approval from System Settings.
- Windows: download the `-setup.exe` when present. The beta is unsigned; SmartScreen may require More info -> Run anyway.
- Linux: download the `.AppImage`, `.deb`, or `.rpm` when present. AppImage files may need `chmod +x`.

Installers contain the app and normal Tauri runtime/installer files. They do not bundle `kubectl`, Helm, Argo CD, kubeconfigs, tokens, or cluster credentials.

## Maintainer Release Flow

1. Update the version in `package.json`, `src-tauri/tauri.conf.json`, and `src-tauri/Cargo.toml`.
2. Merge the version commit to `main`.
3. Run:

```sh
bun run release:dry-run
```

4. If the dry run is clean, create and push the release tag:

```sh
bun run release
```

The release command fetches `origin/main`, reads the release version from that commit, creates an annotated `app-vX.Y.Z` tag pointing at `origin/main`, and pushes only the tag. It can run from any local branch or GitButler workspace.

GitHub Actions runs typecheck, frontend tests, Rust tests, Rust check, builds macOS, Windows, and Linux installers, and publishes a GitHub Release after every platform build succeeds.

Manual workflow dispatch is only for rerunning an existing `app-v*` tag. Reruns preserve the existing release visibility.

## In-App Updates

In-app updates are enabled only for release-workflow builds. Local development, git-pull, and ad hoc builds use the `dev` release channel and do not check GitHub for updates.

Updater artifacts require the GitHub Actions `TAURI_SIGNING_PRIVATE_KEY` secret. `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` is optional and only needed for password-protected keys. The public key is committed in `src-tauri/tauri.conf.json`.

Installer packages may remain OS-unsigned beta builds, but updater artifacts and `latest.json` must be signed with the Tauri updater key.

GitHub release notes should mirror the matching version section in [CHANGELOG.md](../CHANGELOG.md).

## Pre-Release Smoke Test

Automated baseline:

```sh
bun run typecheck
bun test
bun run rust:test
bun run rust:check
```

Manual Tauri path:

1. Start the desktop app with `bun run tauri dev`.
2. Confirm the workspace launcher lists local kube contexts without exposing raw kubeconfig contents.
3. Select a readable context and confirm namespaces load.
4. Open a saved or newly created workspace.
5. Open the resource browser and confirm resources load for the saved scope.
6. Select a resource and confirm details, read-only YAML, events, and logs behave when available.
7. Check Argo CD and Helm sections when the cluster provides matching metadata.

Most recent partial smoke, 2026-05-22:

- `bun run typecheck`, `bun test`, `bun run rust:test`, and `bun run rust:check` passed after repairing a stale `node_modules` install with `bun install --force`.
- `bun run tauri dev` launched KubeCove v0.2.1 and served Vite on `http://localhost:1420/`.
- The Tauri desktop window loaded the readable `admin@solid-k8s` context and listed namespaces including `alloy`, `argocd`, `cert-manager`, `default`, `kube-system`, `monitoring`, and `traefik`.
- Full workspace-open, resource-browser, resource-detail, YAML, events, logs, Argo, and Helm click-through remains the next manual release gate.

## Publishing Checklist

- Confirm the release contains macOS, Windows, and Linux assets.
- Download at least one artifact and confirm it launches.
- Smoke test context listing, namespace/resource browsing, and clean errors when kubeconfig or cluster access is unavailable.
- Share the release only after artifact checks pass.

If a release is bad, leave or restore the prior release as the recommended tester download and delete the broken release after replacing it.
