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

## Publishing Checklist

- Confirm the release contains macOS, Windows, and Linux assets.
- Download at least one artifact and confirm it launches.
- Smoke test context listing, namespace/resource browsing, and clean error messages when kubeconfig or cluster access is unavailable.
- Share the release after artifact checks pass.

If a release is bad, leave or restore the prior release as the recommended tester download and delete the broken release after replacing it.
