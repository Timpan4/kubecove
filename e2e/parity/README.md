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

Native-only tests in `src-tauri/gpui/tests/native.rs` and `src-tauri/gpui/tests/native/` cover what the browser mocks cannot express: read errors, cancellation, stale asynchronous results after scope changes, sessions from other sources, dirty drafts that block closing, refused log streams and dry runs, failed guarded executions, log streams that outlive their detail, dry runs of a draft that changed before they finished, and kubeconfig settings that the native backend cannot save.

`live-data.json` replays the same scripted watch messages in both drivers. It covers resource identity, deletion, reconnect catch-up, stale read failures, permission-limited polling, scope changes, event order, and discovered custom kinds in the sidebar and picker. Native stream tests also check cancellation while opening, dropped streams, and receivers that outlive their page, detail or context. The native client reuses the typed Rust watch commands, the web's 15,000 ms recovery interval from `resource-watch.ts`, and its 250 ms refresh throttle from `live-view-refresh.ts`. Pending row patches apply at rendering without rebuilding the table's filtered rows.

The ignored `watch_burst_frame_time` native test measures a burst against the 10,000-row fixture from `visual/export-watch-burst.ts`. It reports offscreen debug CPU frame construction and executor drain time, not GPU presentation or a release frame rate. The measured frames exceed the 8.33 ms requirement. Live-data scenarios do not prove 120 fps.

The detail-tab scenarios run guarded operations only against the browser mocks. The fixture backend answers a preview or execution only when its request matches the exported one exactly, so a wrong target or a missing guard field fails the scenario instead of reaching a cluster.

## Failure modes

Write acceptance scenarios before implementing the behavior they cover. The current scenarios target wrong-resource selection when kinds share a name, missing YAML identity, a detail panel that cannot close, an empty search that retains old rows, refresh that resets the search, event sort order and type filters, GitOps counts, cluster-scoped listings, an incident guide that ignores the signal filter, port forwards hidden by the source filter, aggregated log lines that lose their order, log filters and level toggles that hide the wrong lines, a log stream that survives a resource switch, a revision diff against the wrong comparison, a review that shows the wrong target or diff, a delete that executes without the typed name, a scale review with the wrong replica count, a dry run that skips the server, YAML apply offered for a Secret, RBAC risks that are not inherited from bindings and roles, a risk filter that keeps the wrong objects, an inspector or access check for the wrong subject, a workspace that opens or reopens with the wrong context or namespaces, kubeconfig settings that do not survive leaving Settings, settings search that keeps non-matching categories, and palette results or targets that ignore the query.

Keep report files under `e2e/artifacts/`. Each report names the driver and scenario IDs it actually ran. Missing evidence is a failure.

The topology-metrics scenarios cover namespace-scoped Network Flow edges, switching between Network Flow and Ownership, node selection, an empty graph, resource metric identity and units, missing samples, and the app process usage footer. Resource metrics render in the resource-page availability card, CPU and Memory table columns, and map-node hints. Resource detail matches Svelte and has no metrics section. The footer reads app process usage separately and its setting lasts for the session. The visual harness captures `network-flow`, `metrics`, and `usage-footer`; passing screenshots and mock scenarios do not prove the 4,000-node map meets the 8.33 ms frame budget.

## Native gaps

The native client does not yet implement these product areas: persisting saved workspaces and RBAC review records beyond the session, workspace import, export and editing, multi-context workspace reads, and the settings rows stored in browser local storage, which render disabled with their reason; exec; starting, reconnecting and stopping port forwards; the Actions tab's image, environment, resource, label, annotation and revision-rollback rows, YAML formatting and the YAML shape and encoding pickers, and the settings that skip review, choose the log layout or force conflicts globally; connected Argo CD profiles and Argo CD Refresh, Sync and rollback, which render disabled; the Argo CD tab's YAML editor views, which show plain text, and its diff syntax highlighting; persisted GitOps and Helm view modes; remote connections; native file dialogs; application updates; read-only MCP settings; and platform packaging.

The workspace-surface scenarios also check that review notes stay with the selected object and workspace, that clearing a review clears its editor, that manual dispositions preserve risk flags, and that palette kind entries open the requested kind. Unsupported settings retain their row and control shapes. Production kubeconfig writes are disabled until the native host initializes the existing Rust settings storage path; the fixture backend tests the save and reload interaction without touching real settings.

This list records the delivery boundary so far. It is not an exhaustive inventory of every interaction in those areas. Full parity requires acceptance scenarios for the complete product surface.
