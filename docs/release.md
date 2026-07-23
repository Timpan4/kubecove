# Beta Releases

KubeCove beta installers are published as GitHub Releases:

https://github.com/Timpan4/kubecove/releases

Use a release installer when testing the app. Build from source only for development work.

Latest published release: [`0.9.1`](https://github.com/Timpan4/kubecove/releases/tag/app-v0.9.1).

Current `main` keeps `0.9.1` metadata until a release PR prepares the next version. Features merged after the `0.9.1` tag are not present in those installers.

## Installer Guide

- macOS: download the `.dmg`. The beta is unsigned; macOS may require right-click Open or approval from System Settings.
- Windows: download the `KubeCove_*_x64-setup.exe` / `-setup.exe` NSIS installer. The beta is unsigned; SmartScreen may require More info -> Run anyway.
- Linux: download the `.AppImage`, `.deb`, or `.rpm` when present. AppImage files may need `chmod +x`.

Installers contain the app and normal Tauri runtime/installer files. They do not bundle `kubectl`, Helm, Argo CD, kubeconfigs, tokens, or cluster credentials.

Windows releases intentionally publish the NSIS setup executable, not MSI, so in-app updates stay on one installer path. Users who installed an older MSI build may see one prompt to uninstall that MSI; after uninstalling it once and installing the NSIS setup executable, future in-app updates should not require that migration step.

## Published 0.9.1 Product Safety

Published `0.9.1` is inspection-first and includes guarded Pod and selector-backed Service port-forward sessions, exact-Pod guarded exec sessions, selected-resource YAML apply, scale for Deployments and StatefulSets, rollout restart for Deployments, StatefulSets, and DaemonSets, and exact delete for Pods and ConfigMaps. It supports local cluster browsing, resource details, YAML, events, logs, metrics, topology, Kubernetes-API-first Argo CD and Flux inspection, Helm release inspection and reconciliation, RBAC summaries, pinned and recent workspace entry points, and Deployment revision history.

Published `0.9.1` does not include connected Argo CD transport, Argo operations, transient Secret-key reveal, the deeper RBAC cockpit, or the production-shaped E2E lab added later on `main`. Pod and selector-backed Service port-forwarding follows [ADR 0003](decisions/0003-guarded-live-sessions.md), guarded resource operations follow [ADR 0004](decisions/0004-guarded-cluster-operations.md), exact-Pod exec follows [ADR 0005](decisions/0005-guarded-pod-exec-sessions.md), and selected-resource YAML apply follows [ADR 0006](decisions/0006-guarded-selected-resource-yaml-apply.md).

## Current Main Release Preparation

Current source adds connected Argo CD inspection and allowlisted operations under [ADR 0013](decisions/0013-argocd-connected-inspection-and-operations.md), runtime Secret disclosure under [ADR 0014](decisions/0014-runtime-secret-disclosure.md), deeper RBAC inspection, and the production-shaped E2E lab. These become release features only after a release PR updates version metadata and `CHANGELOG.md`.

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
bun run e2e:fast
bun run e2e:real -- --kubernetes 1.34 --provider docker
bun run e2e:real -- --kubernetes 1.35 --provider docker
bun run e2e:real -- --kubernetes 1.36 --provider docker
```

The full Kubernetes matrix must pass before installer builds begin. The release workflow also verifies macOS, Windows, and Linux installer assets, updater signatures, `latest.json`, and final updater platform coverage before publishing.

The official tested Kubernetes support window is 1.34–1.36. It advances only through a maintainer PR after matching digest-pinned Kind images exist and the replacement matrix passes, as defined by [ADR 0011](decisions/0011-rolling-kubernetes-support.md). The release matrix uses the full production-shaped lab from [ADR 0012](decisions/0012-production-shaped-e2e-lab.md). [Cilium 1.19.6](https://docs.cilium.io/en/stable/network/kubernetes/requirements/) has upstream-tested coverage through Kubernetes 1.34, and [Argo CD 3.4](https://argo-cd.readthedocs.io/en/stable/operator-manual/installation/#tested-versions) has upstream-tested coverage through Kubernetes 1.35. Later-minor jobs are KubeCove compatibility evidence, not extensions of upstream support claims.

## Manual Smoke Test

Published `0.9.1` baseline:

1. Install and start the release artifact for the target platform. Use `bun run tauri dev` only for development smoke before a release PR.
2. Confirm the workspace launcher lists local kube contexts without exposing raw kubeconfig contents.
3. Select a readable context and confirm namespaces load.
4. Open a saved or newly created workspace.
5. Open the resource browser and confirm resources load for the saved scope.
6. Select a resource and confirm details, YAML, events, and logs behave when available.
7. For a selected Pod, confirm guarded exec requires explicit target and command confirmation, starts, streams output, accepts stdin, resizes, and stops.
8. For a non-Secret selected resource, confirm YAML apply requires the Apply-friendly view, runs dry-run, shows a diff, and requires explicit final confirmation.
9. Confirm guarded scale, rollout restart, and exact delete show exact targets, complete server-side previews, and require final confirmation.
10. Check Argo CD and Helm sections when the cluster provides matching metadata.

Before the next release, also verify:

1. Connected Argo CD requires an explicit profile and transport selection, exposes managed-resource comparison, and never falls back silently.
2. Connected refresh, sync/retry, rollback, terminate, and server-reported resource actions require successful preflight and exact confirmation.
3. Runtime Secret reveal stays per-key and transient; connected Argo Secret payloads remain redacted.
4. RBAC observed grants retain provenance and explicit permission verification remains user-submitted.

Latency smoke:

1. Open Settings -> Diagnostics.
2. Enable diagnostics, clear the trace, and return to the app.
3. Run the smoke path above in the installed app.
4. Return to Settings -> Diagnostics and copy the redacted latency report.
5. Attach or paste the report into the release smoke notes. Treat it as a recorded artifact for now; open an issue for obvious stalls or regressions instead of using a hard publish budget.

Local frontend comparison:

```sh
bun run perf:frontend
bun run perf:resource-scope
```

Windows smoke note: foreground the Tauri window before click-through. The WebView appears as an opaque `WRY_WEBVIEW` pane to UI Automation, so treat UIA as launch/foreground help rather than full interaction automation.

## Publishing Checklist

- Confirm the release PR version and changelog are accurate before merge.
- Confirm release notes distinguish newly released behavior from prior `0.9.1` installers.
- Confirm every claimed operation has its typed command, preview or preflight, confirmation UX, and matching ADR coverage.
- After publish, optionally download one artifact and smoke test context listing, namespace/resource browsing, and clean errors when kubeconfig or cluster access is unavailable.

If a release is bad, run the **Yank Release** workflow for the bad `app-v*` tag. Use `draft` to hide the release while keeping artifacts, or `delete-release` to remove the GitHub Release. Then ship a patch release through the normal release PR flow.
