# AI agents (MCP)

KubeCove can serve read-only tools to AI agents such as Claude Code, Codex and Cursor through the Model Context Protocol (MCP). Agents get the same health, incident, ownership and GitOps evidence the app shows, without `kubectl` and without any way to change the cluster. MCP is off by default in the desktop app and on the server.

## Tools

| Tool | Returns |
| --- | --- |
| `find_problems` | Unhealthy Pods, workloads, Jobs, PersistentVolumeClaims and, without a namespace, Nodes, with their signals and latest Warning event, most severe first. Start here. |
| `list_contexts` | Kubeconfig contexts and which one is current. |
| `list_namespaces` | Namespaces in a context. |
| `list_resource_kinds` | Built-in and custom resource kinds. |
| `list_resources` | Compact rows for one kind: status, readiness, restarts, age, health and owner. `health=unhealthy` keeps only problems. |
| `describe_resource` | One resource's labels, owners, spec, status and recent events. |
| `get_resource_yaml` | One resource as YAML without `managedFields`. |
| `get_pod_logs` | The last lines of one container's log; `previous=true` reads the crashed instance. |
| `get_resource_metrics` | CPU and memory usage from the metrics API, highest CPU first. |
| `list_argocd_applications` | Argo CD Applications with sync status and health. |
| `list_helm_releases` | Helm releases with chart, revision and status. |

Omit `context` to use the kubeconfig's current context. `list_resource_kinds`, `list_resources`, `find_problems` and `get_resource_metrics` return 500 rows per page by default; pass `limit` and `offset` to page through the rest. `get_resource_metrics` pages its pods, workloads and nodes lists separately, so pass `list` with `offset`. The other list tools return every row and take no paging arguments. Logs default to the last 200 lines and allow at most 1,000.

Secret values are always redacted. Anyone holding the MCP token can still read what your Kubernetes identity can read, including ConfigMaps and Pod logs, so treat the token like a kubeconfig. Cluster content such as logs, events and annotations can be written by anyone with write access to the cluster; agents are told to treat it as data, not instructions.

## Desktop app

1. Open **Settings > AI agents** and turn on **MCP server**. KubeCove listens on `http://127.0.0.1:47615/mcp` while the app is open.
2. Select **Copy Claude Code command** and run it in a terminal. It registers the endpoint with its bearer token:

   ```sh
   claude mcp add --transport http kubecove http://127.0.0.1:47615/mcp --header "Authorization: Bearer <token>"
   ```

   For other agents, select **Copy token** and configure an HTTP MCP server with the header `Authorization: Bearer <token>`.

The listener accepts only loopback connections and refuses requests from web pages. Agents read through the app's own caches, so repeated questions about a scope you have open are fast. Change **MCP port** if 47615 is in use; an error appears in Settings when the port cannot be opened. **Rotate token** replaces the token immediately, so update every agent you configured.

## Server

Start `kubecove-server` with `--mcp` to serve the tools at `/mcp` on its existing listener. At startup it prints the endpoint and the path of its token file, `<data-dir>/mcp-token`, but not the token. Read the token from that file on the server host, then register the endpoint with your agent, for example:

```sh
claude mcp add --transport http kubecove https://kubecove.example.com/mcp --header "Authorization: Bearer <token>"
```

The MCP token opens only `/mcp`, and the browser sign-in token, cookies and OIDC sessions do not open `/mcp`. TLS, `--allow-remote` and the Host checks apply unchanged. With Cloudflare Access, agents also need an Access service token. `--mcp` cannot be combined with `--oidc-kubernetes-credentials`, because agent requests carry no user's identity to forward. To rotate the token, stop the server, delete `mcp-token` and start it again.

Tool calls are logged as `command_completed` events with command `mcp.<tool>`, and requests without the token as `request_refused` with reason `mcp_token`. Arguments, results and tokens are never logged.
