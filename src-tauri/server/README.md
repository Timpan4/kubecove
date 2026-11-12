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

The server reads Kubernetes configuration on its own host. It uses exactly one authentication mode: a per-start token (default), Cloudflare Access, or OIDC. Browsers support all three; the desktop remote client supports token and OIDC ([ADR 0031](../../docs/decisions/0031-gpui-remote-sign-in.md)). In token mode, startup prints a login URL containing a fresh token. Open that URL in a browser, or enter the token in the connection screen. The browser removes the token fragment after loading. The token also lives in the server data directory as `server-token`, with Unix mode `0600`. Treat startup output as a credential.

For SSH forwarding, keep the server on loopback. This example uses the same port on both machines.

```sh
ssh -N -L 8080:127.0.0.1:8080 user@server
```

Open `http://127.0.0.1:8080` locally. If the local port differs, start the server with `--public-origin http://127.0.0.1:<local-port>` so Host, Origin and WebSocket CSP checks accept that exact origin.

The default listener is `127.0.0.1:0`, which chooses an available port. Non-loopback TCP requires `--allow-remote`, `--tls-cert` and `--tls-key`. `--socket` accepts a Unix socket path and requires `--public-origin` for its forwarded browser origin. Run `kubecove-server --help` for the remaining options.

Clients run in independent sessions. Reconnects retain their session during `--session-grace`, which defaults to `5m`. Explicit disconnect or grace expiry closes only that session's streams, exec sessions and port-forwards. Browser forwards bind on the server host, so they require separate forwarding to reach them from a laptop. Kubeconfig sources are shared operator configuration in every authentication mode. Set `KUBECONFIG` before starting the server; clients cannot change the shared source settings. A request that names any other kubeconfig environment variable or source fails with a `kubeconfig` error before that variable is read.

## MCP

`--mcp` serves read-only MCP tools for AI agents at `/mcp` on the main listener ([ADR 0028](../../docs/decisions/0028-read-only-mcp-server.md)). `src/mcp.rs` mounts the `kubecove-mcp` endpoint in the router. Requests need the bearer token stored in `<data-dir>/mcp-token`, created on first start with mode `0600` and reused afterwards. Without `--quiet`, startup prints its path, never the token. The MCP token opens only `/mcp`. The router guard still applies the Host policy, the Origin policy when an `Origin` header is present, and Cloudflare Access when configured. Tool calls run in one session scope with the operator's kubeconfig source. With OIDC browser sign-in, `/mcp` needs only the MCP token, not an OIDC session, so treat that file as a service credential. `--mcp` with `--oidc-kubernetes-credentials` fails at startup.

## Logs

Log events follow [ADR 0027](../../docs/decisions/0027-server-observability.md). `src/observe.rs` is the only module that emits them, and it owns the redaction rules. Events go to stderr as text by default, or as one JSON object per line with `--log-format json`. `RUST_LOG` sets levels; the default is `warn,kubecove_server=info`. Each event has an `event` field: `server_started`, `server_failed`, `session_opened`, `session_resumed`, `session_closed`, `session_expired`, `session_cleanup_failed`, `sign_in`, `authentication_refused`, `request_refused`, `oidc_refresh`, `command_completed`, `metrics_listening` or `metrics_listener_failed`. Set `RUST_LOG=kubecove_server=debug` to add successful commands, token renewals and the Host and Origin values of refused requests. The startup login URL stays on stdout.

## Metrics

`--metrics-listen <IP:PORT>` starts a second plain-HTTP listener that serves only `GET /metrics`, in OpenMetrics text format. It has no authentication, so bind it to an address only your scraper can reach. Without the flag nothing listens. `src/metrics.rs` owns the registry and the scrape handler; `observe.rs` updates the event counters. Metric names start with `kubecove_server_`, except `process_resident_memory_bytes` and `process_cpu_seconds_total`. Labels never carry subjects, session ids, clusters, contexts, namespaces or resource names.

Cache counters come from the backend cache diagnostics. Settings -> Diagnostics "Clear" and session end do not lower them: Clear only moves the baseline of the diagnostics table, and an ended session's totals move into a retired sum.

`deploy/grafana/kubecove-server.json` is a Grafana dashboard for these metrics. `src/metrics_dashboard_tests.rs` parses each query and checks it against a scrape: every metric must be exported, every filter and grouping label must exist on the metric it applies to, a selector's fixed label values must occur together on one sample of that metric, and every legend label must survive the query's aggregations. Renaming a metric, a label or a label value fails `cargo test` until the dashboard follows.

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

The server uses native credential storage when available. Unix hosts without it use the encrypted file store approved in [ADR 0021](../../docs/decisions/0021-file-credential-store-without-keyring.md).
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
Desktop clients sign in through the system browser with the native flow in
[ADR 0031](../../docs/decisions/0031-gpui-remote-sign-in.md). The [deployment guide](../../deploy/README.md#openid-connect-with-authentik)
has an Authentik example behind Cloudflare Tunnel.

Releases publish Linux archives and container images. See
[Install the server](../../docs/wiki/Server-Installation.md) for asset names and image tags.
[verification.md](verification.md) holds the evidence recorded before the first server release.
