# ADR 0035: GPUI E2E flavor and agent automation endpoint

## Status

Accepted on 2026-11-15. Extends [ADR 0010](0010-e2e-only-wdio-security-boundary.md) and [ADR 0029](0029-gpui-native-client-and-feature-parity.md). The product owner asked for a way for agents to operate the native client through MCP, as a step toward making it the only desktop client.

## Context

The Tauri client has real-cluster and packaged-app tests: WDIO drives its webview through the E2E-only plugin of ADR 0010. The native client has none. Its parity driver renders offscreen against controlled backend responses, which proves UI wiring but not Kubernetes access. Agents working on the native client also cannot look at the running app or operate it. They can only write a parity scenario and read its report.

GPUI Kit's `test-support` feature already provides what a driver needs. Every element the client marks with `.test_support()` (551 places today) records its path, accessible role, label, description, value, state and bounds in each frame. Text is recorded with its bounds only where an element opts in with `.test_text()` or uses GPUI Kit's inline text; the client has no such element today, so its content reaches a driver through labels and values, such as table cells, which expose their text as labels. GPUI's accessibility tree is private to the platform adapter. Pointer, wheel and key events go through GPUI's own dispatch and hit testing. On macOS and Windows a window can render its current scene to an image. These functions work on any `Window`, not only test windows. ADR 0029 forbids native automation hooks in production builds, and ADR 0010 requires an ADR update and a production absence check for any desktop automation surface.

## Decision

### The E2E flavor

The native client gets a Cargo feature `e2e`. It alone compiles GPUI Kit's `test-support`, the automation endpoint below and the kubeconfig pin. The release workflow builds without it.

Startup checks run before any window, storage or network access:

- An `e2e` build requires `KUBECOVE_E2E=1` and refuses to start otherwise.
- A build without the feature refuses to start when `KUBECOVE_E2E=1` is set, so a test can never believe it drives a production build.
- `KUBECOVE_KUBECONFIG` must name an existing regular file by absolute path, and `KUBECOVE_DATA_DIR` an existing directory by absolute path.
- `KUBECOVE_AUTOMATION_PORT` must be a port number. `0` asks the OS for a free port.
- With `KUBECOVE_E2E_CLUSTER` set, the kubeconfig must hold exactly that lab cluster and its admin and restricted contexts, as in ADR 0010. Without it, the kubeconfig must hold at least one cluster, and every cluster must be an HTTPS API server on `127.0.0.1`, `localhost` or `::1`. In both cases the cluster must be reached directly: a cluster `proxy-url`, `HTTPS_PROXY` or `https_proxy` is refused, because kube-rs would send the request through that proxy, and loopback there names the proxy's host. This accepts a kind or lab cluster and refuses a kubeconfig that also reaches a remote cluster. The proxy variables are checked at startup. Clients re-read the pinned file later, so every read after startup checks again that each cluster is a direct HTTPS loopback server. A file rewritten while the app runs to name a remote or proxied cluster fails that read instead of reaching it.

The pin comes from the shared backend. The `kubecove-kube` crate's E2E kubeconfig handling moves behind a Tauri-free `pinned-kubeconfig` feature, which the Tauri `e2e` feature still includes. While `KUBECOVE_E2E=1`, every cluster read and write uses only the pinned file. `KUBECONFIG`, `~/.kube/config` and paths saved in settings are ignored, and a pinned file that stops loading is an error, not a fallback.

The data directory replaces the platform config directory for kubeconfig settings, the ADR 0030 preference document, saved workspaces and the read-only MCP settings and token. The legacy live-session file is not read. Each launch needs its own data directory. Two launches on one directory would share these files and its token file, and the later one replaces the earlier one's token.

### The automation endpoint

The flavor serves MCP on its own listener at `http://127.0.0.1:<port>/mcp`. It reuses the ADR 0028 endpoint code: Streamable HTTP in stateless JSON mode, a bearer token compared in constant time, loopback `Host` values only, and every request with an `Origin` refused. The read-only MCP listener and its tool list are unchanged, and automation tools never appear there.

Each launch binds the listener, then writes a new 32-byte token from the OS CSPRNG to `<data dir>/automation-token`, mode `0600` on Unix. The app prints the endpoint URL and the token file path on standard output, never the token. The listener stops when the app quits. A bind failure stops the app before it opens a window and leaves an earlier token file in place, so a second launch on a taken port does not lock out the first launch's clients.

| Tool | Effect |
| --- | --- |
| `snapshot` | Draws a frame and returns the window size, the observed elements with a reference, path, role, label, description, value, placeholder, state and bounds, and any recorded text with bounds. Hidden elements are left out unless asked for. |
| `click` | Moves the pointer to a target and presses and releases a mouse button, once or several times. |
| `type_text` | Optionally clicks a target's center to focus it, then types printable text into the focused element one character at a time. Control characters such as a newline or tab are refused; they are keys. |
| `press_key` | Sends one GPUI keystroke: modifiers and one character or a key name GPUI knows, such as `enter`, `escape`, `tab`, `f5` or `ctrl-a`. GPUI's parser accepts any word and would type it, so other words are refused. |
| `scroll` | Sends a wheel event with a pixel delta at a target. |
| `screenshot` | Returns the window as a PNG image, on macOS and Windows. |

A target is a reference from the latest snapshot, an exact accessible label with an optional description, or a window position in pixels. A reference names its snapshot, such as `s3e12`, and works only until the next snapshot from any client, because all clients share one reference table. It resolves by its element path, and only while the window lists the same elements, by path, role, label and description, as at that snapshot. Element ids in the client can hand a path to another item, such as a row index after a row is removed, so a path alone could hit the item that replaced the original. Values and bounds are not compared, so typing or scrolling within the same elements keeps references valid. A label must match exactly one visible element. A target must be visible, which means inside the window and its clip, not uncovered: an element under an overlay such as the workspace launcher is still listed, and a click on it lands on the overlay, as a person's would. Keys and text are parsed before any event is sent, so an invalid request sends nothing. Every failure is a tool error, and the app keeps running.

Events go through the same window dispatch as platform input, so hit testing, focus and key bindings behave as for a person. The endpoint runs each request on the UI thread between frames, in arrival order. A request whose caller stopped waiting is skipped, and one already running sends no further event.

GPUI's Linux platform cannot read back a window's pixels, so `screenshot` reports an error there. On Linux the snapshot's text and bounds are the agent's view of the screen.

### What an agent can do

Through the endpoint, an agent can do anything a person can do in the window, against the pinned kubeconfig. That includes guarded operations and their confirmations, such as delete, scale and YAML apply. The loopback-only cluster rule and the explicit kubeconfig keep direct cluster access to clusters the tester created. Two window features reach beyond the kubeconfig: a remote KubeCove server connection (ADR 0031) and a connected Argo CD instance (ADR 0013). Each reaches only a server whose address and credentials are typed into the window. The endpoint supplies neither, so a test reaches one only when it gives the agent those details. ADR 0028's rule that agent write tools need their own ADR still holds for the product: the automation endpoint exists only in a test build.

Native file dialogs, the platform clipboard and OS menus are outside the window and the endpoint does not drive them. A link control, such as the documentation link, still hands its URL to the OS opener, as it would for a person. Remembered Argo CD credentials still go to the OS keyring, as in the Tauri E2E flavor; a test that remembers one leaves an entry there.

## Consequences

Agents can open the native client against a disposable cluster, read its screen as structured data and operate it. The Tauri real-cluster and desktop specs can be ported onto this endpoint, which is the remaining evidence before the native client can replace the Tauri desktop app.

The flavor compiles a second feature set of the client and its backend. CI checks it with clippy and runs its tests. Building with `test-support` makes every observed element record its facts each frame, so measurements of this flavor do not represent release performance.

## Verification

Failure modes, each covered before release:

1. An `e2e` build without `KUBECOVE_E2E=1` exits before opening a window.
2. A build without the feature exits when `KUBECOVE_E2E=1` is set. The release workflow runs each platform's release binary this way and fails if it starts.
3. A missing or relative kubeconfig or data directory, or a missing or invalid port, exits with the variable's name.
4. Without `KUBECOVE_E2E_CLUSTER`, a kubeconfig with any non-loopback or non-HTTPS cluster, or with no cluster, exits. In either rule, a cluster `proxy-url` or an HTTPS proxy variable exits.
5. Requests without the automation token, with a wrong token, with a non-loopback `Host` or with any `Origin` run no tool. The shared endpoint tests of ADR 0028 cover this code.
6. A reference from an earlier snapshot, or taken before the window's elements changed, including one whose path now names another row, an ambiguous or missing label, a hidden target, an invalid key or an untypeable character returns an error and dispatches no event.
7. `screenshot` on Linux returns an error and the app keeps running.
8. The token file is created with mode `0600` and replaced at every launch that binds its port, and kept when the bind fails.
9. A cancelled request dispatches no further event.
