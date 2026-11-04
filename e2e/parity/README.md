# Frontend feature parity

The Svelte web frontend and the GPUI native client share one set of UI acceptance tests. Each file in `scenarios/` declares scenarios, and both drivers run every scenario against the same data:

- `web.ts` drives the Svelte app in Chromium with the browser mocks from `src/lib/tauri-dev-mocks.ts`.
- `src-tauri/gpui/tests/parity.rs` drives the GPUI client offscreen with `visual/export-fixture.ts`'s export of those same mock responses, through `src-tauri/gpui/tests/common/`.

`bun run e2e:parity` exports the fixture, runs both drivers and passes only when both report every scenario as passed. A passing mock scenario proves UI integration through the command seam, not a real Kubernetes request or a packaged application.

## Equal-tests rule

A UI change in either client lands with a shared scenario that both drivers pass. `contract.test.ts` checks two invariants on every `bun test` run:

1. Every view in `visual/views.ts` has at least one scenario whose ID starts with `<view>.`. Adding a native view to the visual harness without a shared scenario fails the check.
2. Every action in `ACTIONS` (`contract.ts`) has a handler in both `web.ts` and `parity.rs`.

An action names user-visible behavior, not a selector. Each driver maps it to its own UI: GPUI test IDs and labels, and web roles, names and text. When an action cannot be expressed in both clients, the clients have diverged and the divergence is the bug to fix.

Native-only tests in `src-tauri/gpui/tests/native.rs` and `src-tauri/gpui/tests/native/` cover what the browser mocks cannot express: read errors, cancellation, stale asynchronous results after scope changes, sessions from other sources, dirty drafts that block closing, refused log streams and dry runs, failed guarded executions, log streams that outlive their detail, and dry runs of a draft that changed before they finished.

The detail-tab scenarios run guarded operations only against the browser mocks. The fixture backend answers a preview or execution only when its request matches the exported one exactly, so a wrong target or a missing guard field fails the scenario instead of reaching a cluster.

## Failure modes

Write acceptance scenarios before implementing the behavior they cover. The current scenarios target wrong-resource selection when kinds share a name, missing YAML identity, a detail panel that cannot close, an empty search that retains old rows, refresh that resets the search, event sort order and type filters, GitOps counts, cluster-scoped listings, an incident guide that ignores the signal filter, port forwards hidden by the source filter, aggregated log lines that lose their order, log filters and level toggles that hide the wrong lines, a log stream that survives a resource switch, a revision diff against the wrong comparison, a review that shows the wrong target or diff, a delete that executes without the typed name, a scale review with the wrong replica count, a dry run that skips the server, and YAML apply offered for a Secret.

Keep report files under `e2e/artifacts/`. Each report names the driver and scenario IDs it actually ran. Missing evidence is a failure.

## Native gaps

The native client does not yet implement these product areas: saved workspaces and settings; live watch recovery; full dynamic-resource discovery; live event watches and metrics; the network-flow topology mode; exec; starting, reconnecting and stopping port forwards; the Actions tab's image, environment, resource, label, annotation and revision-rollback rows, YAML formatting and the YAML shape and encoding pickers, and the settings that skip review, choose the log layout or force conflicts globally; the GitOps, Argo CD, Flux and Helm pages; RBAC; remote connections; native file dialogs; application updates; read-only MCP settings; and platform packaging.

This list records the delivery boundary so far. It is not an exhaustive inventory of every interaction in those areas. Full parity requires acceptance scenarios for the complete product surface.
