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

The [resource observation module](src/inspector/resource_observation.rs) owns the
shown resource rows, their list versions, initial watch versions, and held watch
patches. Context arrivals, custom-kind reads, refreshes, and watch patches install
their evidence through that owner. Presentation reads its rows and status without
mutating snapshot state. Stream tasks use the existing Backend adapters; topology
reads retain their separate owner. Domain terms are defined in the
[glossary](../../GLOSSARY.md).

RBAC inspection, exact access verification, workspace scope selection, settings
search and command-palette navigation have shared scenarios. Preferences, saved
workspaces with their RBAC review notes, and port-forward presets persist in one
versioned local document (see
[ADR 0030](../../docs/decisions/0030-gpui-local-preferences.md) and
[`src/preferences/README.md`](src/preferences/README.md)). Documents from a remote
connection last for the session only. Production initializes the desktop's existing
kubeconfig settings file. File selection and workspace import/export use native
dialogs behind `Backend`; tests supply controlled responses. Remote connections
sign in with a token or OIDC
([ADR 0031](../../docs/decisions/0031-gpui-remote-sign-in.md)) and reuse the Rust
HTTP/WebSocket client, with no local fallback after socket failure. Explicit
disconnect returns to local access.

Update checks read `latest-gpui.json` beside the desktop feed on the same release.
Install downloads this platform's build, checks its signature and signed version
against the key in `tauri.conf.json`, and replaces the AppImage or app bundle, or
saves the Windows installer and runs it on relaunch. See
[ADR 0034](../../docs/decisions/0034-gpui-release-bundles-and-updates.md).
Settings rows the native host cannot back stay visible, disabled, with the reason.
Settings scenarios use worktree-local temporary storage; native storage tests
reopen it in separate processes.

The Actions tab supports image, environment, requests and limits, labels, annotations
and revision rollback through the existing typed preview and execution commands.
Every change keeps its target scope and server review. YAML supports Kubectl and
Apply clean shapes, YAML and KYAML encodings, and formatting in a background task.
Discovered resources read their YAML through the existing dynamic-details command.
Editor entries use virtual lists. The skip-change-review, log-layout and YAML
force-conflict settings are stored with the other preferences.

See [FRAME_TIMING.md](FRAME_TIMING.md) for the optimized frame harness and measurement boundaries.

## E2E flavor and agent automation

The `e2e` feature builds a test flavor that agents can operate through MCP
([ADR 0035](../../docs/decisions/0035-gpui-e2e-flavor-and-agent-automation.md)). It reads
only the kubeconfig you name, and every cluster in it must be an HTTPS API server on
loopback, such as the one `bun run dev:kind` creates. It keeps its settings in the data
directory you name, not in your config directory. Release builds do not have it.

```sh
export KUBECOVE_E2E=1
export KUBECOVE_KUBECONFIG=/absolute/path/to/kubeconfig
export KUBECOVE_DATA_DIR=/absolute/path/to/empty/directory
export KUBECOVE_AUTOMATION_PORT=0   # 0 picks a free port
cargo run --locked --manifest-path src-tauri/gpui/Cargo.toml --features e2e
```

The app prints `[kubecove:automation] MCP endpoint http://127.0.0.1:<port>/mcp; bearer
token in <data dir>/automation-token`. The token changes at every launch. To give Claude
Code the tools for that launch:

```sh
claude mcp add --transport http kubecove-ui http://127.0.0.1:<port>/mcp \
  --header "Authorization: Bearer $(cat "$KUBECOVE_DATA_DIR/automation-token")"
```

Start with `snapshot`, which lists the window's elements with references, roles, labels,
values and states. Most visible content, such as table cells, appears as labels and
values. Then use `click`, `type_text`, `press_key` and `scroll` on a reference, an
accessible label or a window position. Elements under an overlay, such as the workspace
launcher, are still listed, but a click on them lands on the overlay. `screenshot`
returns a PNG on macOS and Windows; GPUI's Linux windows cannot capture their pixels.
Native file dialogs are not reachable through the endpoint.

The workspace pins GPUI Kit 0.9.0 with components, assets and the YAML grammar.
Test support is enabled only for development builds. The backend dependency
uses the existing GUI-free configuration.
