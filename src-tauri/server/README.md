# Headless server

This crate builds the existing backend with its `desktop` feature disabled. It owns network authentication, session ownership, HTTP command dispatch, WebSocket channels, and bundled frontend serving. Kubernetes logic stays in the parent crate.

The transport and lifecycle rules are defined by [ADR 0020](../../docs/decisions/0020-headless-server-mode.md).

OIDC sign-in and token validation live in `src/oidc.rs`. Token renewal and session
credential bindings live in `src/oidc_refresh.rs`.

Build from the repository root. The web build must precede Cargo because the server embeds `dist`.

```sh
bun install --frozen-lockfile
KUBECOVE_PUBLIC_RUNTIME=web bun --bun vite build
cargo build --locked --release --manifest-path src-tauri/server/Cargo.toml
src-tauri/server/target/release/kubecove-server --listen 127.0.0.1:8080
```

The server reads Kubernetes configuration on its own host. It uses exactly one authentication mode: a per-start token (default), Cloudflare Access, or OIDC. Browsers support all three; the native desktop remote client supports token mode only. In token mode, startup prints a login URL containing a fresh token. Open that URL in a browser, or enter the token in the connection screen. The browser removes the token fragment after loading. The token also lives in the server data directory as `server-token`, with Unix mode `0600`. Treat startup output as a credential.

For SSH forwarding, keep the server on loopback. This example uses the same port on both machines.

```sh
ssh -N -L 8080:127.0.0.1:8080 user@server
```

Open `http://127.0.0.1:8080` locally. If the local port differs, start the server with `--public-origin http://127.0.0.1:<local-port>` so Host, Origin and WebSocket CSP checks accept that exact origin.

The default listener is `127.0.0.1:0`, which chooses an available port. Non-loopback TCP requires `--allow-remote`, `--tls-cert` and `--tls-key`. `--socket` accepts a Unix socket path and requires `--public-origin` for its forwarded browser origin. Run `kubecove-server --help` for the remaining options.

Clients run in independent sessions. Reconnects retain their session during `--session-grace`, which defaults to `5m`. Explicit disconnect or grace expiry closes only that session's streams, exec sessions and port-forwards. Browser forwards bind on the server host, so they require separate forwarding to reach them from a laptop. Kubeconfig sources are shared operator configuration in every authentication mode. Set `KUBECONFIG` before starting the server; clients cannot change the shared source settings.

## Memory and transport

Binaries use the platform's system allocator. A mimalloc global allocator was
measured on the Linux desktop host and raised its resident memory by up to about
160 MiB, so it is not used.

`KUBECOVE_SERVER_WORKER_THREADS` accepts a positive integer to set Tokio's async
worker count. Invalid values fail startup. When unset, Tokio retains its default
of one worker per available CPU, or its existing `TOKIO_WORKER_THREADS` override.
The KubeCove variable takes precedence. The runtime's blocking pool is separate.
No lower default is selected without a throughput comparison: prior replay measured
allocator sensitivity, not throughput parity across worker counts. Operators can
use this override to compare their I/O workload with fewer workers.

The web build precompresses embedded Vite assets with gzip and Brotli. Only smaller
compressed representations are embedded. Successful files under `/assets/` matching
Vite's default eight-character hash suffix receive
`Cache-Control: max-age=31536000, immutable`. Encoding negotiation honors
`Accept-Encoding` qualities and compressed-encoding exclusions, prefers Brotli at
equal quality, and falls back to identity if no acceptable encoding is available.
It sets `Vary: Accept-Encoding`. HTML, API responses, errors and other files retain
`no-store`. The existing Host, Origin, authentication and CSP guards still wrap
asset requests.

Each live channel queue retains at most 1,000 messages and one desktop WebSocket
frame's payload capacity, the existing 2 MiB frame limit minus the channel JSON
wrapper, including during disconnect grace. Oldest messages are evicted to fit both
bounds. A message larger than the byte budget is dropped without closing its
producer or discarding smaller pending messages. The existing `channel_gap` frame
reports drops at their position among retained messages. Gap counts use fixed
metadata per queued message and coalesce consecutive drops. These bounds apply per
channel, not per session or process.
The replay budget is one frame of serialized payload plus the 1,000-message cap,
so long log lines leave fewer lines retained across a reconnect; a `channel_gap`
marker reports the drops.

HTTP command results serialize directly from their typed values to JSON bytes.
Channel payloads are serialized once on enqueue; desktop remote channels forward
raw JSON without constructing a payload tree. WebSocket compression remains
unavailable because [Tungstenite does not support permessage-deflate](https://github.com/snapview/tungstenite-rs#features).

The server uses native credential storage when available. Unix hosts without it use the encrypted file store approved in [ADR 0021](../../docs/decisions/0021-file-credential-store-without-keyring.md). Desktop remote-client integration and release packaging are tracked in [verification.md](verification.md).

Local transport verification uses an isolated Kubernetes WebSocket echo fixture. It creates no cluster resources.

```sh
cargo test --locked --manifest-path src-tauri/server/Cargo.toml
cargo clippy --locked --manifest-path src-tauri/server/Cargo.toml --all-targets -- -D warnings
cargo build --locked --manifest-path src-tauri/server/Cargo.toml
bun e2e/headless-smoke.ts
```

The smoke report is written to `e2e/artifacts/headless/report.json`.

To also exercise the actual Rust desktop remote client against that same isolated
fixture without a display, run `KUBECOVE_NATIVE_REMOTE_PROBE=1 bun e2e/headless-smoke.ts`.
This builds the desktop test target and requires its native build dependencies.
It checks laptop listener binding, duplex bytes, upload EOF and tunnel cleanup.

## OpenID Connect

OIDC browser authentication follows [ADR 0022](../../docs/decisions/0022-oidc-server-sign-in.md).
Configure `--oidc-issuer`, `--oidc-client-id`, `--oidc-login-timeout`, native TLS and
an exact HTTPS `--public-origin`. Confidential clients additionally mount a private
`--oidc-client-secret-file`. The issuer application must register
`<public-origin>/api/v1/oidc/callback` and restrict permitted users. See the
[operator instructions](../../docs/wiki/Remote-Server.md#sign-in-through-openid-connect).

OIDC is mutually exclusive with Cloudflare Access. It generates no startup token.
By default it authenticates KubeCove users only; Kubernetes uses the server's
credentials. With `--oidc-kubernetes-credentials <AUD>`, Kubernetes instead receives
each session's verified user ID token. Transient provider outages suspend expired
tokens and retry within the existing sign-in deadline. Browser logout revokes the
current sign-in, clears its bound Kubernetes bearer slots and cookies, and closes
the session identified by a matching resume token. Other sockets using that sign-in
close and follow the disconnect grace policy; separate sign-ins remain valid.
Native desktop remote connections
require token mode. The [deployment guide](../../deploy/README.md#openid-connect-with-authentik)
has an Authentik example behind Cloudflare Tunnel.

No release has published server archives or a container image yet. Build from source
as above, or build the image described in [deploy/README.md](../../deploy/README.md).
