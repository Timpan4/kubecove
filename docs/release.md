# Beta Releases

KubeCove beta installers are published as GitHub Releases:

https://github.com/Timpan4/kubecove/releases

Use a release installer when you only want to test the app. Building from source is for development work.

## Installer Guide

- macOS: download the `.dmg` asset. The beta is unsigned, so macOS may require right-clicking the app and choosing Open, or allowing it from System Settings.
- Windows: download the `-setup.exe` asset when present. The beta is unsigned, so SmartScreen may show a warning; choose More info and Run anyway if you trust the build.
- Linux: download the `.AppImage` for a portable build, or `.deb` / `.rpm` for package-manager installs when those assets are present. AppImage may need `chmod +x`.

The beta bundles only the app and normal Tauri installer/runtime files. It does not bundle `kubectl`, Helm, Argo CD, kubeconfig files, tokens, or cluster credentials.

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

The release command can run from any local branch or GitButler workspace. It fetches `origin/main`, reads the release version from that remote commit, creates an annotated `app-vX.Y.Z` tag pointing at `origin/main`, and pushes only the tag.

GitHub Actions then runs typecheck, frontend tests, Rust tests, Rust check, builds macOS, Windows, and Linux installers, and publishes a GitHub Release after every platform build succeeds. Manual workflow dispatch is only for rerunning an existing `app-v*` tag; reruns preserve the existing release visibility.

In-app updates are enabled only for builds produced by the release workflow. Local development, git-pull, and ad hoc builds default to the `dev` release channel and do not check GitHub for updates.

In-app updates are built for macOS, Windows, and Linux from the same release workflow. They require the GitHub Actions `TAURI_SIGNING_PRIVATE_KEY` secret to contain the updater private key content. `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` is optional and only needed when the key was generated with a password. The public key is committed in `src-tauri/tauri.conf.json`; the Windows updater install mode in that config only controls Windows installer behavior. The installer packages may remain OS-unsigned beta builds, but updater artifacts and `latest.json` must be signed with the Tauri updater key.

Release notes in GitHub Releases should mirror the matching version section in
the root `CHANGELOG.md`.

## Pre-Release Smoke Test

Use automated checks during feature PRs. After the current seven-PR roadmap has
merged, run the manual Tauri smoke test as a final release gate before cutting a
beta. Record the exact context and result in the release PR or release notes.

Baseline checks:

```sh
bun run typecheck
bun test
bun run rust:test
bun run rust:check
```

Manual Tauri path:

1. Start the desktop app with `bun run tauri dev`.
2. Confirm the workspace launcher loads local kube contexts without exposing raw
   kubeconfig contents.
3. Select a readable context and confirm namespaces load.
4. Open a saved or newly created workspace.
5. Open the resource browser and confirm resources load for the saved scope.
6. Select a resource and confirm details, read-only YAML, events, and logs
   surfaces behave as expected when available.
7. Check Argo CD and Helm sections when the cluster provides matching metadata.

Most recent partial smoke, 2026-05-22:

- `bun run typecheck`, `bun test`, `bun run rust:test`, and `bun run rust:check`
  passed locally after repairing a stale `node_modules` install with
  `bun install --force`.
- `bun run tauri dev` launched KubeCove v0.2.1 and served Vite on
  `http://localhost:1420/`.
- The Tauri desktop window loaded the readable `admin@solid-k8s` context and
  listed namespaces including `alloy`, `argocd`, `cert-manager`, `default`,
  `kube-system`, `monitoring`, and `traefik`.
- The full workspace-open, resource-browser, resource-detail, YAML, events,
  logs, Argo, and Helm click-through is deferred until after the seven roadmap
  PRs merge.

## Publishing Checklist

- Confirm the release contains macOS, Windows, and Linux assets.
- Download at least one artifact and confirm it launches.
- Smoke test context listing, namespace/resource browsing, and clean error messages when kubeconfig or cluster access is unavailable.
- Share the release after artifact checks pass.

If a release is bad, leave or restore the prior release as the recommended tester download and delete the broken release after replacing it.
