# Frontend feature parity

`inspection.json` defines the same acceptance scenarios for the Svelte web frontend and the GPUI native client. Each driver must execute every step and assert the specified outcome. Unknown actions fail. A passing mock scenario proves UI integration through the command seam, not a real Kubernetes request or a packaged application.

## Failure modes

Write acceptance scenarios before implementing the behavior they cover. The initial scenarios target wrong-resource selection when kinds share a name, missing YAML identity, a detail panel that cannot close, an empty search that retains old rows, and refresh that resets the search. Native-specific coverage must also exercise errors, cancellation, stale asynchronous results after scope changes, and local draft diffs that cannot mutate a cluster.

Keep report files under `e2e/artifacts/`. Each report names the driver and scenario IDs it actually ran. The parity gate requires matching successful reports for all declared scenarios. Missing evidence is a failure.

## Native gaps

The first native slice covers resource inspection. The following product areas remain open until implemented and verified: saved workspaces and settings; live watch recovery; full dynamic-resource discovery; live event watches and metrics; the network-flow topology mode; logs and exec; starting, reconnecting and stopping port forwards; guarded operations and server-produced YAML review; the GitOps, Argo CD, Flux and Helm pages; RBAC; remote connections; native file dialogs; application updates; read-only MCP settings; and platform packaging.

The native client also has read-only versions of the ownership map, resource events and timeline, the Incident Cockpit, the Port Forwards session list, the Cluster Overview and the overview's Argo CD and Flux counts. Native UI tests and `bun scripts/gpui-visual.ts` cover them. None is parity-covered until `inspection.json` has scenarios for it and both drivers pass them.

This list records the initial delivery boundary. It is not an exhaustive claim that every interaction in those areas has been inventoried. Full parity requires acceptance criteria for the complete product surface.
