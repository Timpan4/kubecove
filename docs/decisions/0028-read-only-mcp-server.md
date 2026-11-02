# ADR 0028: Read-Only MCP Server

## Status

Accepted and implemented on 2026-11-03.

## Context

AI agents such as Claude Code, Codex and Cursor reach external systems through the Model Context Protocol (MCP). Agents that troubleshoot Kubernetes today usually shell out to `kubectl`, which returns raw objects, has no notion of KubeCove's health assessment, incident signals or GitOps ownership, and can mutate the cluster. KubeCove already computes the evidence an agent needs and holds warm watch caches and clients for the contexts the user works in.

MCP makes the command surface reachable by a new client that is not the bundled frontend. [ADR 0020](0020-headless-server-mode.md) and the product trust boundary assume the only clients are the bundled frontend and the desktop remote client. An agent is a different kind of client: it acts on model output, and the cluster content it reads (logs, events, annotations, CRD fields) is text that anyone with write access to the cluster can control. The product document requires a fresh ADR before AI-assisted operations; this decision covers read-only inspection only.

## Decision

### Read-only tool set

KubeCove serves MCP tools that read Kubernetes state through the existing command functions and their cluster-access intents ([ADR 0026](0026-cluster-access-by-intent.md)). Every tool uses the Read intent. No tool previews, applies, scales, restarts, deletes, syncs, port-forwards, execs, reveals Secret values or changes settings. Each tool is annotated `readOnlyHint: true`, `destructiveHint: false`, `idempotentHint: true`.

The phase 1 tools are:

| Tool | Backed by |
| --- | --- |
| `list_contexts` | `list_kube_contexts` |
| `list_namespaces` | `list_namespaces` |
| `list_resource_kinds` | `list_resource_kinds` |
| `list_resources` | `list_resource_scope`, with kind resolution through discovery |
| `find_problems` | `list_incident_cockpit` over the workload, Pod and PersistentVolumeClaim kinds, plus Nodes for a scan of all namespaces |
| `describe_resource` | `get_resource_details` or `get_dynamic_resource_details`, plus `list_resource_events` |
| `get_resource_yaml` | `get_resource_yaml` or `get_dynamic_resource_details` |
| `get_pod_logs` | a finite Pod log read in `kubecove-live` |
| `get_resource_metrics` | `list_resource_metrics` |
| `list_argocd_applications` | `list_argocd_applications` (Kubernetes transport) |
| `list_helm_releases` | `list_helm_releases` |

Tools are declared in one module of the `kubecove-mcp` crate. They are not generated from the [ADR 0025](0025-single-command-contract.md) contract, because that contract includes mutating, streaming and desktop-only commands, and an agent-facing tool needs a compact result and its own description. A tool can only call a command function; adding a tool never adds a command.

Connected Argo CD transport is out of scope. It needs a per-session connection and credential that an agent session does not have.

### Compact, bounded results

Tools return JSON text that keeps the evidence an agent reasons with: name, namespace, status, readiness, restarts, age, health state and reasons, owner and GitOps ownership. They drop `managedFields`, full health evidence trees and repeated cluster names.

List-style tools (`list_resource_kinds`, `list_resources`, `find_problems`, and each list of `get_resource_metrics`) accept `limit` and `offset` and report `total` and `nextOffset`. The default page size is 500 rows, chosen by the product owner on 2026-11-02. `get_pod_logs` reuses the existing log stream bounds: a default tail of 200 lines, a maximum of 1,000 lines and the existing per-line byte cap.

`find_problems` returns active signals first and omits resolved and historical signals unless the agent asks for them.

### Redaction and untrusted content

- Secret `data` and `stringData` are redacted by the same functions the frontend uses ([ADR 0014](0014-runtime-secret-disclosure.md)). `reveal_secret_data_value` has no tool.
- Kubeconfig contents, tokens, certificate data and connected Argo CD credentials never appear in tool results.
- The server instructions and each tool description state that cluster content is untrusted data, not instructions. Results are data from the cluster; KubeCove does not try to filter prompt injection out of them.
- Errors are returned as the existing `AppError` message and kind. They already exclude credentials (ADR 0026).

### Hosts and transport

Both hosts serve MCP over Streamable HTTP in stateless JSON mode. Each request is independent and carries its own bearer token. No MCP session state is kept between requests.

**Desktop.** Settings > AI agents enables a listener on `127.0.0.1:<port>/mcp` inside the running app. The default port is 47615, chosen by the product owner on 2026-11-02, and is editable. The listener binds loopback only and runs only while the app is open. Tool calls use the app's own `ClusterLiveStore` and client caches, so an agent reads the same warm caches as the UI. Desktop MCP uses the local kubeconfig sources and the Local source policy; it does not proxy through a desktop remote connection.

**Server.** `kubecove-server --mcp` serves `/mcp` on the existing listener, so TLS, `--allow-remote` and the Host allowlist apply unchanged. Requests with an `Origin` header must match the served origin, as browser `POST` requests do. With Cloudflare Access, `/mcp` also requires a valid Access assertion, so agents need an Access service token in addition to the MCP token. Without `--mcp`, the server has no MCP route. MCP requests share one session scope and one `ClusterLiveStore` created at startup, with the operator's source policy. `--mcp` with `--oidc-kubernetes-credentials` fails at startup, because MCP requests carry no user's Kubernetes identity and that mode strips the operator's credentials. With OIDC browser sign-in, `/mcp` does not require an OIDC session: the MCP token is an operator-held service credential, accepted by the product owner on 2026-11-03, and agents cannot complete a browser sign-in.

### Authentication

MCP is disabled by default on both hosts.

- Each host authenticates MCP requests with a dedicated bearer token of 32 bytes from the OS CSPRNG, the same strength ADR 0020 requires for the server token. Comparison is constant-time.
- The token is stored as `mcp-token` in the host's data directory (the desktop app config directory, or the server `--data-dir`), mode `0600` on Unix, and is reused across restarts so agent configuration stays valid. It is created on first use: when MCP starts, or when the desktop user first copies or rotates it.
- The desktop never displays the token. Settings copies it, alone or inside a Claude Code command, only when the user asks, and can rotate it. Rotation restarts the listener, so the old token stops working immediately. The server prints the token file path, not the token, at startup; operators rotate it by deleting the file and restarting.
- The MCP token grants only `/mcp`. It does not authenticate `/api/*`, and browser credentials (server token, cookies, Cloudflare Access or OIDC sessions) do not authenticate `/mcp`.
- The desktop listener also rejects any `Host` other than its loopback address and any request carrying an `Origin`, so a web page cannot reach it through DNS rebinding or a cross-origin `fetch`.

### Observability

The server logs MCP tool calls and refused MCP requests through `observe.rs` ([ADR 0027](0027-server-observability.md)). A tool call is a `command_completed` event whose command is `mcp.<tool>`, or `mcp.unknown` for an undeclared name, so the existing command metrics and dashboard cover it. A request without the MCP token is a `request_refused` event with reason `mcp_token`. Arguments, resource names, results and tokens are not logged. Desktop finite reads continue to appear in Settings > Diagnostics because tools run through `FiniteRead`.

## Consequences

Agents get KubeCove's health, incident, ownership and GitOps evidence in one call each, without `kubectl` and without any write path. The desktop serves agents from the caches the UI already holds, so repeated questions about the same scope do not reload it.

The trust boundary gains one authenticated, read-only client type per host. Anyone holding the MCP token can read what the host's Kubernetes identity can read, including ConfigMaps and Pod logs that may contain sensitive data. Secret values stay redacted.

A workspace change in the desktop rotates the client generation and can cancel an in-flight MCP read; the agent receives a cancellation error and can retry.

Write tools, Connected Argo CD, per-user server identity for MCP (OAuth), stdio transport, topology and RBAC review tools are out of scope. Any write tool needs its own ADR, with confirmation that an agent cannot supply on its own.

## Verification

Failure modes to cover before release:

- `/mcp` missing, wrong or browser-credential authentication is refused without running a tool;
- the MCP token does not authenticate `/api/*`;
- desktop requests with a non-loopback `Host` or any `Origin` are refused; server requests follow the server Host and Origin policy;
- `/mcp` serves no tools without `--mcp`, and desktop MCP is off until enabled;
- `--mcp` with `--oidc-kubernetes-credentials` fails at startup;
- `tools/list` contains only the read-only tools above;
- Secret details and YAML returned by tools contain no Secret values;
- `limit`/`offset` paging returns every row exactly once and reports `nextOffset`;
- the token file is created with mode `0600`, reused across restarts, and rotation rejects the old token;
- a bind failure on the desktop port is shown in Settings and does not stop the app.
