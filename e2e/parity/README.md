# Frontend feature parity

`inspection.json` defines the same acceptance scenarios for the Svelte web frontend and the GPUI native client. Each driver must execute every step and assert the specified outcome. Unknown actions fail. A passing mock scenario proves UI integration through the command seam, not a real Kubernetes request or a packaged application.

## Failure modes

Write acceptance scenarios before implementing the behavior they cover. The initial scenarios target wrong-resource selection when kinds share a name, missing YAML identity, a detail panel that cannot close, an empty search that retains old rows, and refresh that resets the search. Native-specific coverage must also exercise errors, cancellation, stale asynchronous results after scope changes, and local draft diffs that cannot mutate a cluster.

Keep report files under `e2e/artifacts/`. Each report names the driver and scenario IDs it actually ran. The parity gate requires matching successful reports for all declared scenarios. Missing evidence is a failure.

## Native gaps

The first native slice covers resource inspection. The following product areas remain open until implemented and verified: saved workspaces and settings; live watch recovery; full dynamic-resource discovery; events and metrics; topology; logs and exec; port forwarding; guarded operations and server-produced YAML review; GitOps, Argo CD, Flux and Helm; RBAC and incidents; remote connections; native file dialogs; application updates; read-only MCP settings; and platform packaging.

This list records the initial delivery boundary. It is not an exhaustive claim that every interaction in those areas has been inventoried. Full parity requires acceptance criteria for the complete product surface.
