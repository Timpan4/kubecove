# ADR 0027: Server Observability

## Status

Accepted on 2026-11-01. Structured logging is implemented. The metrics listener is accepted and not yet implemented.

## Context

`kubecove-server` wrote a few unstructured stderr lines. Operators running it in Kubernetes had no record of sign-in failures, refused requests, session lifecycle or OIDC token refreshes, and no numbers for open sessions, cache weight or command failures. Issues [#522](https://github.com/Timpan4/kubecove/issues/522) and [#523](https://github.com/Timpan4/kubecove/issues/523) ask for structured logs and Prometheus metrics that share event names.

Logs and metrics leave the server's trust boundary: they go to log collectors and scrapers the operator runs. [ADR 0020](0020-headless-server-mode.md) states that the server's security relies on loopback or TLS, the selected authentication mode and Host/Origin checks. An unauthenticated metrics listener is a new network surface, so it needs this decision. The design and plan are in [server observability design](../server-observability-design.md).

## Decision

### One observation module

`src-tauri/server/src/observe.rs` is the only server module that emits log events or updates metrics. Each observed point is one typed function. The event name is the log `event` field and the metric name stem, so logs and metrics cannot drift apart. Reason and outcome values are Rust enums, so every value is fixed at compile time.

### Redaction

- No event or label carries the startup token, resume token, cookies, Cloudflare Access JWTs, OIDC codes, state, nonce, PKCE verifier, ID, access or refresh tokens, kubeconfig contents, Secret data or resource payloads.
- Errors are recorded by `AppErrorKind` only. `AppError` messages can name resources.
- Each session has a random 64-bit log identifier that is independent of its resume token.
- Subjects appear in logs only, never in metric labels. Client-supplied text (subjects, Host and Origin values) is escaped.
- Command names are the declared names from `command_contract!`, or `unknown`. A request path never becomes a log or label value.
- Host and Origin values from refused requests are logged only when debug output is enabled for `kubecove_server`. At the default level a refusal records its reason and a standard method name.

### Logging

- `--log-format text|json` selects the stderr format. Text is the default. JSON writes one object per line.
- `RUST_LOG` sets levels with `tracing-subscriber` directive syntax. The default is `warn,kubecove_server=info`. An invalid value fails startup. Raising dependency targets to `debug` can log Kubernetes request URIs, which name namespaces and resources.
- The startup login URL, `--quiet` and `--print-token` keep their stdout behavior.
- Successful and cancelled commands log at `debug`. Failed commands log at `warn`.
- The desktop app installs no subscriber.

### Metrics listener

- `--metrics-listen <IP:PORT>` starts a second plain-HTTP listener that serves only `GET /metrics` in Prometheus text format. Without the flag, nothing listens.
- Any address is accepted. The flag is the opt-in, Prometheus scrapes pod IPs, and the content is aggregate counts. Operators restrict scrapers with a NetworkPolicy or firewall.
- The listener shares no route, cookie or authentication state with the main API.
- Labels exclude subjects, session identifiers, clusters, contexts, namespaces and resource names.
- Command durations use the Prometheus `client_golang` default buckets.
- Cache metrics read the backend diagnostics counters from [issue #419](https://github.com/Timpan4/kubecove/issues/419) and stay monotonic across Settings -> Diagnostics "Clear" and session end.

## Consequences

Operators can filter and alert on sign-in failures, refusals and session lifecycle, and collectors can parse JSON lines. Every new observation point must go through `observe.rs`, and tests capture the production JSON formatter to assert that the token and OIDC sign-in paths log no credentials. The metrics port widens the server's network surface by one read-only, unauthenticated endpoint that the operator enables and scopes.

OpenTelemetry export, per-request Kubernetes API client metrics and log shipping are out of scope.
