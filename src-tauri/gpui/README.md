# GPUI client

This opt-in native client runs separately from the Tauri and Svelte client.
Kubernetes access goes through `Backend`, which calls the shared Rust commands.
See [ADR 0029](../../docs/decisions/0029-gpui-native-client-and-feature-parity.md)
and the [shared parity guide](../../e2e/parity/README.md) for the delivery boundary.

From the repository root:

```sh
cargo run --locked --manifest-path src-tauri/gpui/Cargo.toml
bun run e2e:parity
bun scripts/gpui-visual.ts rbac,workspaces,settings,palette
```

The parity and visual drivers render offscreen with controlled backend data.
They do not read the user's kubeconfig or change a cluster. Reports and screenshots
are retained under `e2e/artifacts/`. A passing scenario proves the UI interaction
through the command boundary, not a packaged app or a live Kubernetes request.

RBAC inspection, exact access verification, workspace scope selection, settings
search and command-palette navigation have shared scenarios. Saved workspaces and
RBAC review notes last for the session only. The repository has no Rust storage
commands for these records. Production initializes the desktop's existing
kubeconfig settings file. File selection and workspace import/export use native
dialogs behind `Backend`; tests supply controlled responses. Remote token-mode
connections reuse the Rust HTTP/WebSocket client, with no local fallback after
socket failure. Explicit disconnect returns to local access.

Update checks read `latest-gpui.json` beside the desktop feed on the same release.
Install downloads this platform's build, checks its signature and signed version
against the key in `tauri.conf.json`, and replaces the AppImage or app bundle, or
saves the Windows installer and runs it on relaunch. See
[ADR 0034](../../docs/decisions/0034-gpui-release-bundles-and-updates.md).
Browser-only preferences keep their
controls disabled pending a storage decision. Settings scenarios use worktree-local
temporary storage; native storage tests reopen it in separate processes.

The Actions tab supports image, environment, requests and limits, labels, annotations
and revision rollback through the existing typed preview and execution commands.
Every change keeps its target scope and server review. YAML supports Kubectl and
Apply clean shapes, YAML and KYAML encodings, and formatting in a background task.
Discovered resources read their YAML through the existing dynamic-details command.
Editor entries use virtual lists. Skip-review, log-layout and global force-conflict
settings remain disabled because native browser-local-storage persistence is absent.

See [FRAME_TIMING.md](FRAME_TIMING.md) for the optimized frame harness and measurement boundaries.

The workspace pins GPUI Kit 0.9.0 with components, assets and the YAML grammar.
Test support is enabled only for development builds. The backend dependency
uses the existing GUI-free configuration.
