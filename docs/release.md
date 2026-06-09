# Beta Releases

KubeCove beta installers are published as GitHub Releases:

https://github.com/Timpan4/kubecove/releases

Use a release installer when testing the app. Build from source only for development work.

Current version metadata: `0.6.3`.

## Installer Guide

- macOS: download the `.dmg`. The beta is unsigned; macOS may require right-click Open or approval from System Settings.
- Windows: download the `KubeCove_*_x64-setup.exe` / `-setup.exe` NSIS installer. The beta is unsigned; SmartScreen may require More info -> Run anyway.
- Linux: download the `.AppImage`, `.deb`, or `.rpm` when present. AppImage files may need `chmod +x`.

Installers contain the app and normal Tauri runtime/installer files. They do not bundle `kubectl`, Helm, Argo CD, kubeconfigs, tokens, or cluster credentials.

Windows releases intentionally publish the NSIS setup executable, not MSI, so in-app updates stay on one installer path. Users who installed an older MSI build may see one prompt to uninstall that MSI; after uninstalling it once and installing the NSIS setup executable, future in-app updates should not require that migration step.

## Product Safety

The current beta is inspection-first and includes guarded Pod and selector-backed Service port-forward sessions, exact-Pod guarded exec sessions, and selected-resource YAML apply. It supports local cluster browsing, resource details, YAML, events, logs, metrics, topology, Argo CD inspection, Helm release inspection and reconciliation, RBAC summaries, local-only port-forwarding, guarded exec from a selected Pod, and guarded apply from the selected resource YAML panel.

Broad cluster-changing workflows such as arbitrary apply, delete, scale, sync, and rollback are not release features unless a typed command and guarded UX path exist. Pod and selector-backed Service port-forwarding follows [ADR 0003](decisions/0003-guarded-live-sessions.md), exact-Pod exec follows [ADR 0005](decisions/0005-guarded-pod-exec-sessions.md), and selected-resource YAML apply follows [ADR 0006](decisions/0006-guarded-selected-resource-yaml-apply.md). Future operation releases must follow [ADR 0004](decisions/0004-guarded-cluster-operations.md).

## Maintainer Release Flow

1. Open GitHub Actions and run **Prepare Release PR** with a `patch`, `minor`, or `major` bump.
2. Review the generated release PR:
   - version metadata in `package.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`, `Cargo.lock`, `README.md`, and this guide
   - generated `CHANGELOG.md` section
   - release notes do not claim guarded operations that are not implemented
3. Merge the release PR to `main`.

The release PR is the human review gate. After merge, GitHub Actions finds the merged PR with the `release` label, creates the matching annotated `app-vX.Y.Z` tag, runs the release workflow, builds macOS, Windows, and Linux installers, verifies updater assets, and publishes the GitHub Release automatically.

Manual workflow dispatch for the release workflow is only for rerunning an existing `app-v*` tag. Reruns preserve the existing release visibility.

Local maintainers can validate the current `origin/main` release metadata with:

```sh
bun run release:dry-run
```

`bun run release` no longer creates or pushes tags. Releases start from the GitHub **Prepare Release PR** workflow.

## In-App Updates

In-app updates are enabled only for release-workflow builds. Local development, git-pull, and ad hoc builds use the `dev` release channel and do not check GitHub for updates.

Updater artifacts require the GitHub Actions `TAURI_SIGNING_PRIVATE_KEY` secret. `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` is optional and only needed for password-protected keys. The public key is committed in `src-tauri/tauri.conf.json`.

Installer packages may remain OS-unsigned beta builds, but updater artifacts and `latest.json` must be signed with the Tauri updater key.

GitHub release notes should mirror the matching version section in [CHANGELOG.md](../CHANGELOG.md).

## Automated Release Gates

Every release tag runs:

```sh
bun run typecheck
bun test
bun run rust:test
bun run rust:check
```

The release workflow also verifies macOS, Windows, and Linux installer assets, updater signatures, `latest.json`, and final updater platform coverage before publishing.

## Manual Smoke Test

Manual Tauri path:

1. Start the desktop app with `bun run tauri dev`.
2. Confirm the workspace launcher lists local kube contexts without exposing raw kubeconfig contents.
3. Select a readable context and confirm namespaces load.
4. Open a saved or newly created workspace.
5. Open the resource browser and confirm resources load for the saved scope.
6. Select a resource and confirm details, YAML, events, and logs behave when available.
7. For a selected Pod, confirm guarded exec requires explicit target and command confirmation, starts, streams output, accepts stdin, resizes, and stops.
8. For a non-Secret selected resource, confirm YAML apply requires the Apply-friendly view, runs dry-run, shows a diff, and requires explicit final confirmation.
9. Check Argo CD and Helm sections when the cluster provides matching metadata.

Windows smoke note: foreground the Tauri window before click-through. The WebView appears as an opaque `WRY_WEBVIEW` pane to UI Automation, so treat UIA as launch/foreground help rather than full interaction automation.

Most recent partial smoke, 2026-05-26:

- The local Tauri dev app used `admin@solid-k8s` after KubeCove 0.3.0 was released.
- The workspace launcher loaded the saved workspace, restored the overview, and showed live cluster counts, incident shortcuts, Argo CD summary, and CPU/memory footer.
- The resource browser opened from the overview.
- Pod resource listing remained on `Loading all namespaces` for more than 20 seconds, blocking full table, detail, YAML, event, log, metrics, and topology smoke coverage.
- KubeCove dev mode now uses ports 1430/1431 so it can run alongside another Tauri project on the default 1420/1421 ports.

0.5.0 release-hardening checks should re-run this path on `admin@solid-k8s`, plus guarded Pod exec, selected-resource YAML apply, and Helm reconciliation, before publishing.

## Publishing Checklist

- Confirm the release PR version and changelog are accurate before merge.
- Confirm release notes do not claim guarded operations that are not implemented.
- After publish, optionally download one artifact and smoke test context listing, namespace/resource browsing, and clean errors when kubeconfig or cluster access is unavailable.

If a release is bad, run the **Yank Release** workflow for the bad `app-v*` tag. Use `draft` to hide the release while keeping artifacts, or `delete-release` to remove the GitHub Release. Then ship a patch release through the normal release PR flow.
