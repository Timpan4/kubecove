# ADR 0020: Headless Server Mode

## Status

Accepted 2026-10-09.

Clarified 2026-10-11: multiple user sessions per backend were always the product goal. The single-owner model below is the implemented first stage, not a permanent restriction. [ADR 0023](0023-multi-user-sessions-and-persistence.md) proposes the next stage with persistent user workspaces and settings.

## Context

Some users keep kubeconfig files, exec credential plugins, and network reach to their clusters on a remote development or operations host. The desktop app cannot use that host unless it runs there with a display. Tauri v2 has no supported web or server target, and its IPC reaches only the app's own webview.

The current trust boundary assumes the Rust backend is reachable only through Tauri IPC from the bundled frontend. A server mode makes the same command surface reachable over a network socket, so the boundary needs explicit rules.

## Decision

KubeCove adds `kubecove-server`, a separate binary that runs as one OS user, initially reads that OS user's kubeconfig sources, serves the bundled frontend, and exposes the existing command surface over HTTP and WebSocket. Delivery starts with one active client session per backend. The intended next stage supports multiple users and concurrent client sessions on the same backend, with persistent user workspaces and settings. Running as one OS user does not limit the eventual number of application users. The server does not deploy anything into a cluster, impersonate other identities, or run shell commands.

Amendment, 2026-10-10: operators may deploy this same binary as a container in Kubernetes, including k0s. The repository supplies an image build and a private deployment example. The app still performs no installation or self-deployment. A dedicated service account receives the built-in `view` role, and a mounted kubeconfig references its projected, rotating token and cluster CA. Cluster operations and Secret reads remain unavailable unless the operator explicitly grants their permissions. One replica and the `Recreate` strategy preserve single-owner semantics and prevent concurrent writers to the private data directory. The example uses an operator-provided persistent volume claim and a loopback listener reached through `kubectl port-forward`.

Cloudflare Tunnel and Access can provide browser access while preserving the exact Host/Origin policy. An HTTPS public origin requires server TLS, including when the tunnel connects over pod loopback. A tunnel connector in the same pod can reach that loopback origin without a Service, ingress, NodePort or LoadBalancer. Cloudflare account changes and deployment are operator actions, not app runtime behavior.

Two clients use it in the first release:

- a browser, normally through an SSH local forward;
- the desktop app in remote mode, where desktop Rust acts as the HTTP and WebSocket client. The frontend calls one typed remote-proxy path, and the server token stays in desktop Rust memory for the active connection. The app does not remember remote credentials across restarts or require a laptop keyring for remote sign-in.

### Code layout

Amendment, 2026-10-11: the server's default `web-ui` Cargo feature embeds the browser app. Disabling default features produces an API-only binary without frontend assets or embedding dependencies. Container releases provide the default full image and a `-slim` API-only image. Both retain the same authentication, Host/Origin checks, command table and session rules. Separately hosted browser assets must be routed through the API's exact origin; API-only builds add no CORS permissions.

The existing backend library builds without its default `desktop` feature. Tauri, its plugins, and its build dependency are optional. A small `runtime` module supplies borrowed state and outbound channels for the server, while desktop builds retain the existing Tauri types and command attributes. `kubecove-server` adds HTTP and WebSocket adapters to the same command functions. This keeps Kubernetes behavior shared without moving the backend into another crate. The headless dependency tree must contain no Tauri or webview libraries.

### Transport

- `POST /api/v1/invoke/{command}` accepts the same camelCase JSON arguments as the Tauri command and returns the same result or serialized `AppError`.
- One authenticated WebSocket per client carries stream messages, exec input and output, and port-forward tunnel frames, multiplexed by id.
- Desktop tunnel upload EOF uses an owner-scoped `tunnel_eof` control after the last binary payload. The server drops that connection's upload sender, shuts down the shared Kubernetes forwarding stream's write half, and drains already received response frames before `tunnel_closed`. Unknown IDs cannot open a stream. Kubernetes port-forward protocol support still determines whether a pod can return a response after EOF; KubeCove does not promise TCP half-close support beyond that protocol.
- A pod-side EOF closes the tunnel after queued response frames. A tunnel error with a connection ID is terminal for that connection, including when it occurs after the open handshake.
- The server dispatches through an explicit command table. A test requires every desktop command to be either in that table or in a named desktop-only list.
- Desktop-only commands are the native file dialogs (`pick_kubeconfig_paths`, `save_workspace_export_json`, `pick_workspace_import_json`) and the updater, process, and opener plugins. Browser mode replaces them with a kubeconfig path field that Rust validates, a browser download for workspace export, and a file input for workspace import. Existing Rust-side size limits still apply.

### Network exposure and authentication

- The default listener is loopback TCP. On Unix, a Unix socket with mode `0600` is also supported.
- A non-loopback address requires both `--allow-remote` and a TLS certificate and key. Otherwise the server refuses to start.
- In default token mode, each start generates a fresh token of 32 random bytes from the OS CSPRNG. The server prints a login URL once and writes the token to `server-token` with mode `0600` in the data directory. Tokens never appear in persistent logs or diagnostics.
- A loopback listener still requires the selected authentication method, because other users on a shared host can reach loopback ports.
- In token mode, the browser exchanges the token once for an `HttpOnly`, `SameSite=Strict` cookie, marked `Secure` under TLS, and the login URL is replaced without the token. The desktop client sends the token as a bearer credential. Comparisons are constant-time.
- Every request checks the Host header against loopback names plus any configured public host. Every browser `POST` and WebSocket upgrade checks Origin against the served origin. The server sends no CORS headers, a restrictive Content Security Policy, and `frame-ancestors 'none'`.
- Holding the token grants the server user's cluster access through KubeCove. The guarded operation, live session, and Secret disclosure rules in ADRs 0003 through 0006, 0013, and 0014 apply unchanged.

### Initial-stage session ownership and reconnect

These rules describe the implemented single-owner stage. They remain in force until an accepted follow-up decision and its implementation replace them.

- The server has one active owner at a time. Authenticating another client makes it the owner. The previous socket receives `session_moved` and closes. Its streams, exec sessions, and tunnelled forwards stop. Port-forwards listening on the server move to the new owner, where they stay visible and stoppable.
- Each owner receives a resume token. If the socket drops, the owner's streams, exec sessions, and port-forwards keep running for the grace window. Stream output is buffered within existing per-stream bounds, dropping oldest first. Reconnecting with the resume token reattaches channels and redraws exec terminals.
- `--session-grace` sets the window. The default is 5 minutes, chosen by the product owner. A value of `0` stops everything on disconnect. When the window expires, cleanup matches desktop app exit.
- The owner holds backend state, channels and tunnels. The initial stage deliberately shares the server-bound port-forward registry across owner takeovers. Multiple simultaneous sessions require explicit isolation of that registry and of any process-wide settings or credential state; existing owner checks alone do not prove multi-user isolation.

### Cloudflare Access sign-in

Amendment, 2026-10-10: the operator can select Cloudflare Access authentication with both `--cloudflare-access-team` and `--cloudflare-access-aud`. In this mode, Access sign-in is the only browser login. KubeCove automatically opens its session without a server token. The default remains token authentication for private remote hosts and native desktop clients.

- Access mode requires native TLS and an exact HTTPS public origin. It accepts only a signed `Cf-Access-Jwt-Assertion`, verified with RS256 against the configured team's HTTPS `/cdn-cgi/access/certs` endpoint. Verification requires the exact issuer and application audience, an unexpired application token, and a nonempty subject. Unverified email headers, token bodies, KubeCove cookies and bearer tokens cannot authenticate Access mode.
- The application audience is Cloudflare's AUD tag, not the Access application UUID. The operator's Access policy determines which users can access this shared backend. KubeCove continues to use the server's Kubernetes identity rather than impersonating the email address.
- Protected HTTP requests and WebSocket upgrades require a valid Access JWT. The authentication-mode endpoint exposes only the selected mode and requires no credential. Open WebSockets close when the verified token expires; resources follow the existing reconnect grace policy. Refreshed Access tokens with the same subject can reconnect. A session resume credential cannot be used by a different Access subject. A fresh login can transfer the single active owner, as before.
- Signing keys refresh from the configured endpoint when their HTTP cache lifetime expires or an unknown key ID is received. Redirects and JWT-supplied key URLs are not followed. Key-fetch or validation failure denies access; expired keys are not used as a fallback.
- The server exposes its authentication mode, never credentials, so the browser can choose automatic Access sign-in or the existing token form. In Access mode it generates no startup token, writes no token file and sets no KubeCove authentication cookie. Access credentials never enter application logs or diagnostics.

Boundary verification must cover valid Access login without a token, forged or duplicate assertion headers, wrong signature, algorithm, issuer or audience, expired/not-yet-valid JWTs, unknown or rotated signing keys, unavailable key service, token/cookie fallback, missing or cross-origin requests, cross-subject resume/invoke/close, WebSocket expiration, and preservation of token mode. Browser verification must reach the workspace without rendering the token form.

### Live sessions, data, and diagnostics

OIDC browser sign-in is an additional mutually exclusive mode defined in
[ADR 0022](0022-oidc-server-sign-in.md). It authenticates users without changing
the server's Kubernetes identity or initial-stage single-owner contract.

- In browser mode, port-forward listeners bind to `127.0.0.1` on the server, and the UI labels them as remote. Desktop remote mode tunnels forwards to the laptop under the ADR 0003 amendment of 2026-10-09.
- Server-side kubeconfig-source settings use `--data-dir`, which defaults to the platform config directory. Kubeconfig paths refer to server files, and Rust returns only summaries. Saved workspaces and UI settings currently use client `localStorage`; navigation snapshots use client `sessionStorage`. They are not yet a durable per-user server store.
- Connected Argo CD credentials and operation review sessions use native credential storage when available, otherwise the file store in [ADR 0021](0021-file-credential-store-without-keyring.md).
- Usage and diagnostics report the server process. The server does not self-update.

## Consequences

Remote hosts become first-class without a display server or webview libraries on that host. The command surface gains a second transport, so command contracts, error shapes, and stream semantics must stay transport-neutral and are checked by the parity test. The core extraction touches every command module but changes no behavior on its own. Security relies on loopback or TLS, the selected token or verified Access authentication mode, and Host and Origin checks. Removing any of them requires amending this ADR.

The single-owner stage establishes remote transport and reconnect behavior before concurrent-user isolation and durable user storage. It completes that first stage without completing the multi-user goal. The next stage must distinguish authenticated user identity, client-session ownership, Kubernetes credential scope, and persisted user data.

Verification runs the existing E2E specs through `kubecove-server` in a browser and through the desktop remote client. Isolated tests cover the boundary: missing or wrong token, wrong Host, cross-origin `POST` or WebSocket, non-loopback bind without TLS, desktop-only commands over HTTP, oversized bodies, stale resume tokens, and tunnel frames for unknown or other-owner ids.
