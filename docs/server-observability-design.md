# Server observability design

Design and implementation plan for [#522](https://github.com/Timpan4/kubecove/issues/522) (structured and JSON logging) and [#523](https://github.com/Timpan4/kubecove/issues/523) (Prometheus metrics) in `kubecove-server`. Status: implemented. Owner decisions are recorded at the end.

## Baseline before this work

- The server wrote four unstructured lines: the startup login URL (`main.rs`, stdout), `--help`, and two cleanup failures (`sessions.rs`, stderr). Sign-in failures, Host/Origin refusals, session lifecycle and OIDC refreshes left no record.
- `tracing` 0.1.44 was already in the server lockfile through `kube`, `hyper` and `tower`. No subscriber was installed, so those events were dropped.
- Cache counters (#419) live per session in `ClusterLiveStore` (`crates/kube/src/commands/live_store_diagnostics.rs`). They count unconditionally; only the snapshot command is gated by the session's diagnostics toggle. Settings -> Diagnostics "Clear" stored 0 into them.
- Concurrent sessions (#521) were merged. Each `Owner` holds its own `BackendState`, so cache counters disappeared when a session ended.
- Every command passes through one seam: `invoke()` in `server/src/lib.rs` calling `dispatch::dispatch`.

## Design

### One observation module, two outputs

Add `server/src/observe.rs`. It is the only server module that calls `tracing` macros or touches metric handles. Each lifecycle point calls one typed function, for example `observe::session_opened(&owner, mode)`, which emits the log event and updates the matching metric. This is how logs and metrics share event names (#522 asks for that), and it keeps redaction rules in one file a reviewer can read.

Event names (log `event` field, metric name stem):

| Event | Log level | Fields | Metric |
| --- | --- | --- | --- |
| `server_started` | info | version, auth_mode, transport (tcp, tls, unix); emitted once the listener and TLS configuration are ready | none |
| `server_failed` | error | error (the startup or serve error that stops the server) | none |
| `metrics_listening` | info | address | none |
| `metrics_listener_failed` | warn | error_kind | none |
| `session_opened` | info | session_id, auth_mode, subject | `kubecove_server_sessions_opened_total{auth_mode}` |
| `session_resumed` | info | session_id, auth_mode, subject | `kubecove_server_sessions_resumed_total{auth_mode}` |
| `session_closed` | info | session_id, reason (client, logout, shutdown, activation_failed) | `kubecove_server_sessions_closed_total{reason}` |
| `session_expired` | info | session_id, grace_seconds | `kubecove_server_sessions_expired_total` |
| `session_cleanup_failed` | warn | session_id, error_kind | `kubecove_server_session_cleanup_failures_total{error_kind}` |
| `sign_in` | info / warn | auth_mode, outcome, reason, subject on success | `kubecove_server_sign_ins_total{auth_mode,outcome,reason}` |
| `authentication_refused` | info | auth_mode, reason (missing, invalid, unavailable) | `kubecove_server_authentication_refused_total{auth_mode,reason}` |
| `request_refused` | warn | reason (host, origin, mcp_token), method; host and origin at debug | `kubecove_server_requests_refused_total{reason}` |
| `oidc_refresh` | debug / warn | outcome (renewed, refused, unusable, failed) | `kubecove_server_oidc_refreshes_total{outcome}` |
| `command_completed` | debug for ok and cancelled, warn otherwise | session_id, command, outcome, error_kind, duration_ms | `kubecove_server_command_duration_seconds{command,outcome}` and `kubecove_server_command_errors_total{command,error_kind}` |

`sign_in` covers the token-mode login request and the OIDC callback. Cloudflare Access signs users in before requests reach the server, so its rejected assertions appear as `authentication_refused`.

`reason` and `outcome` values are Rust enums in `observe.rs`, so every label value is fixed at compile time.

### Redaction rules

These extend the backend diagnostics rules (#419):

- Never log or label: startup token, resume token, cookies, Access JWTs, OIDC codes, state, nonce, PKCE verifier, ID, access or refresh tokens, kubeconfig contents, Secret data, resource payloads.
- Log `AppError::kind` only, never `AppError` itself. Its `Debug` output includes `message`, which can carry resource names.
- `session_id` is a random 64-bit value generated when the session opens and stored on `Owner`. It is not derived from the resume token. See decision 4.
- `subject` appears in logs only, never in metric labels.
- Header values from refused requests (Host, Origin) are attacker-supplied text, bounded only by the HTTP parser. They are logged, escaped, only when debug output is enabled for `kubecove_server`, so the default level cannot be used to write large values into logs.
- The command label is the declared command name from `command_contract!`, or `unknown`. The path segment is never used as a label, because any authenticated caller can send arbitrary names.

### Logging (#522)

- Dependencies: `tracing` (pin to the lockfile's 0.1.44), `tracing-subscriber` 0.3.23 (latest stable, 2026-03-13) with `fmt`, `json`, `env-filter`, `std`, `tracing-log`, default features off.
- `--log-format text|json`, default `text`. Both write to stderr. JSON uses one flattened object per line.
- Filtering reads `RUST_LOG`. The default directive is `warn,kubecove_server=info`. Raising dependency targets to `debug` can log Kubernetes request URIs, which include namespaces and resource names. The operator docs state this.
- The startup login URL, `--quiet` and `--print-token` stay as `println!` on stdout, unchanged. The two `eprintln!` cleanup lines become `session_cleanup_failed`.
- The subscriber is installed in `main.rs` only. The desktop app links no subscriber and no new dependency.

### Metrics (#523)

- Dependency: `prometheus-client` (the Prometheus project's Rust client, `github.com/prometheus/client_rust`). Its latest release, 0.25.2, came out 2026-10-30; pin the newest 0.25.x at implementation time. It supports scrape-time custom collectors, which the cache and session gauges need. `sysinfo` is reused at the workspace's pinned 0.39.6.
- `--metrics-listen <IP:PORT>`, off by default. It starts a second `axum::serve` with a router that has only `GET /metrics`. That router shares no state with the main router except the metrics registry and a read handle to `Sessions`. No sign-in, cookie, or main-API route is reachable through it.
- Scrape-time values:
  - `kubecove_server_sessions_open`: count of live owners.
  - `kubecove_server_cache_{hits,misses,joins,evictions,restored_reload_failures}_total{cache}` and `kubecove_server_cache_{ready_entries,dirty_entries,loading_entries,retained_items,shallow_payload_bytes}{cache}`. `cache` is one of the six fixed labels.
  - `process_resident_memory_bytes` and `process_cpu_seconds_total` from `sysinfo` (`Process::memory()`, `Process::accumulated_cpu_time()` in CPU-milliseconds). Both work on Linux, macOS and Windows.
- Families with labels appear in a scrape after their first sample. `kubecove_server_sessions_open`, `kubecove_server_sessions_expired_total` and the process metrics are always present.
- Excluded labels: subject, session id, cluster, context, namespace, resource name.

#### Keeping cache counters monotonic

The issue requires reusing the diagnostics counters. Two current behaviors would make a Prometheus counter go down, and `rate()` reads every decrease as a process restart:

1. "Clear" in Settings stored 0. It now records a per-counter baseline. The diagnostics snapshot reports `raw - baseline`, so desktop and browser diagnostics behave as before, and `ClusterLiveStore::lifetime_diagnostics()` returns the raw values.
2. Ending a session dropped its counters. `Sessions` now keeps `RetiredCaches`, the summed raw totals of ended sessions. Close, expiry and shutdown remove the owner from the live list under the transition lock, then call `end()`, which finishes cleanup and adds the owner's totals to the retired sum. Counts that a stopping stream task records after that point are not exported; the counters undercount slightly but never decrease. A scrape reads the retired sum before the live owners, so a session can be missing from one scrape while it ends but is never counted twice. Each exported counter is raised to the scraped total and never lowered, so a session missing from one scrape cannot make a counter go down.

#### Histogram buckets

`prometheus-client` has no default bucket set. The proposed set is Prometheus `client_golang`'s `DefBuckets` (5 ms to 10 s, 11 buckets), a documented default other exporters use. See decision 2.

## Failure modes the tests must catch

Logging:

- A secret appears in any event: startup token, resume token, cookie, Access JWT, OIDC code, ID token or refresh token. The sign-in test drives token login (wrong then right token) and an OIDC callback through `oidc_mock.rs`. It captures JSON output with a scoped subscriber and asserts that no secret substring appears.
- A required lifecycle event is missing or emitted twice: open, resume, close, expiry, shutdown, cleanup failure.
- `--log-format json` writes a line that is not a single JSON object.
- `--quiet` or `--print-token` output changes.

Metrics:

- Any label value is a subject, session id, cluster, namespace or resource name. The test opens sessions with known subjects, runs commands naming a known namespace and resource, scrapes, and asserts that none of those strings appear.
- A cache counter decreases after Clear or after a session ends.
- A session's counters appear in both the live and the retired totals, or in neither, during a concurrent close and scrape.
- An unknown command name creates a new label value.
- The metrics listener serves `/api/v1/*`, assets, or WebSocket upgrades.
- Without `--metrics-listen`, a port is bound.

## Plan

Two pull requests, in order. Steps 2 and 3 below shipped together, because the cache totals have no consumer without the metrics listener.

1. **ADR and logging (#522).** Write ADR 0027 "Server observability" covering the redaction rules, label rules and the unauthenticated metrics listener. ADR 0020 says removing a security assumption requires an amendment, so the listener needs the ADR before it ships. Add `observe.rs`, `--log-format`, the subscriber in `main.rs`, `session_id` on `Owner`, and the call sites in `lib.rs` (`guard`, `login`, `invoke`, `close_session`, `oidc_logout`), `oidc.rs` (callback), `access.rs`, `oidc_refresh.rs` and `sessions.rs`. Add config tests for the flag and the redaction test.
2. **Monotonic cache totals.** In `crates/kube`, switch Clear to baselines and add `cache_totals()`. The existing diagnostics tests must still pass unchanged, which shows the desktop sees the same behavior.
3. **Metrics (#523).** Add `--metrics-listen`, the registry, the scrape handler, `CacheTotals`, the command histogram in `invoke()`, and the label and listener-isolation tests. Update `deploy/kubernetes/kubecove.yaml` with a commented metrics port, plus `docs/wiki/Remote-Server.md`, `server/README.md` and `--help`.

Checks per PR: `cargo test`, `cargo clippy` and `cargo fmt --check` with `--manifest-path src-tauri/server/Cargo.toml`, and `cargo check --manifest-path src-tauri/Cargo.toml` to confirm the desktop build is unchanged. A manual run checks `--log-format json` output with `jq` and scrapes `/metrics` with `curl`.

Out of scope: OpenTelemetry export, per-request Kubernetes API client metrics (they need a tower layer in `crates/kube`, which the desktop shares), and log shipping.

## Owner decisions

Decided 2026-11-01:

1. **Metrics listener exposure.** `--metrics-listen` accepts any address over plain HTTP. The flag is the opt-in; operators restrict scrapers with a NetworkPolicy.
2. **Histogram buckets.** `client_golang` `DefBuckets`: 5, 10, 25, 50, 100, 250 and 500 ms, then 1, 2.5, 5 and 10 s.
3. **Successful command log level.** `debug`. Failures log at `warn`.
4. **Session id.** A random 64-bit id generated at session open, independent of the resume token. This replaces the issue's "hash of the resume token" wording.
