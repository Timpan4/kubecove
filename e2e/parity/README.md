# Frontend feature parity

The Svelte web frontend and the GPUI native client share one set of UI acceptance tests. Each file in `scenarios/` declares scenarios, and both drivers run every scenario against the same data:

- `web.ts` drives the Svelte app in Chromium with the browser mocks from `src/lib/tauri-dev-mocks.ts`.
- `src-tauri/gpui/tests/parity.rs` drives the GPUI client offscreen with `visual/export-fixture.ts`'s export of those same mock responses, through `src-tauri/gpui/tests/common/`.

`bun run e2e:parity` exports the fixture, runs both drivers and passes only when both report every scenario as passed. A passing mock scenario proves UI integration through the command seam, not a real Kubernetes request or a packaged application.

## Equal-tests rule

A UI change in either client lands with a shared scenario that both drivers pass. `contract.test.ts` checks two invariants on every `bun test` run:

1. Every view in `visual/views.ts` has at least one scenario whose ID starts with `<view>.`. Adding a native view to the visual harness without a shared scenario fails the check.
2. Every action in `ACTIONS` (`contract.ts`) has a handler in both `web.ts` and the modules under `tests/parity/`, dispatched by `parity.rs`.

An action names user-visible behavior, not a selector. Each driver maps it to its own UI: GPUI test IDs and labels, and web roles, names and text. When an action cannot be expressed in both clients, the clients have diverged and the divergence is the bug to fix.

Native-only tests in `src-tauri/gpui/tests/native.rs` and `src-tauri/gpui/tests/native/` cover what the browser mocks cannot express: read errors, cancellation, stale asynchronous results after scope changes, sessions from other sources, dirty drafts that block closing, refused log streams and dry runs, failed guarded executions, log streams that outlive their detail, dry runs of a draft that changed before they finished, and kubeconfig settings that the native backend cannot save. Native preference tests additionally cover file-backed process restart, migration, corrupt or future documents, stale revisions and failed writes. Their paths stay in worktree-local temporary directories.

`live-data.json` replays the same scripted watch messages in both drivers. It covers resource identity, deletion, reconnect catch-up, stale read failures, permission-limited polling, scope changes, event order, and discovered custom kinds in the sidebar and picker. Native stream tests also check cancellation while opening, dropped streams, and receivers that outlive their page, detail or context. The native client reuses the typed Rust watch commands, the web's 15,000 ms recovery interval from `resource-watch.ts`, and its 250 ms refresh throttle from `live-view-refresh.ts`. Pending row patches apply at rendering without rebuilding the table's filtered rows.

The ignored `watch_burst_frame_time` native test measures a burst against the 10,000-row fixture from `visual/export-watch-burst.ts`. It reports offscreen debug CPU frame construction and executor drain time, not GPU presentation or a release frame rate. The measured frames exceed the 8.33 ms requirement. Live-data scenarios do not prove 120 fps.

The detail-tab scenarios run guarded operations only against the browser mocks. The fixture backend answers a preview or execution only when its request matches the exported one exactly, so a wrong target or a missing guard field fails the scenario instead of reaching a cluster.

## Failure modes

Write acceptance scenarios before implementing the behavior they cover. The current scenarios target wrong-resource selection when kinds share a name, missing YAML identity, a detail panel that cannot close, an empty search that retains old rows, refresh that resets the search, event sort order and type filters, GitOps counts, cluster-scoped listings, an incident guide that ignores the signal filter, port forwards hidden by the source filter, aggregated log lines that lose their order, log filters and level toggles that hide the wrong lines, a log stream that survives a resource switch, a revision diff against the wrong comparison, a review that shows the wrong target or diff, a delete that executes without the typed name, a scale review with the wrong replica count, a dry run that skips the server, YAML apply offered for a Secret, RBAC risks that are not inherited from bindings and roles, a risk filter that keeps the wrong objects, an inspector or access check for the wrong subject, a workspace that opens or reopens with the wrong context or namespaces, kubeconfig settings that do not survive leaving Settings, settings search that keeps non-matching categories, and palette results or targets that ignore the query.

Keep report files under `e2e/artifacts/`. Each report names the driver and scenario IDs it actually ran. Missing evidence is a failure.

The topology-metrics scenarios cover namespace-scoped Network Flow edges, switching between Network Flow and Ownership, node selection, an empty graph, resource metric identity and units, missing samples, and the app process usage footer. Resource metrics render in the resource-page availability card, CPU and Memory table columns, and map-node hints. Resource detail matches Svelte and has no metrics section. The footer reads app process usage separately and its setting persists in the native local preference document. The visual harness captures `network-flow`, `metrics`, and `usage-footer`; passing screenshots and mock scenarios do not prove the 4,000-node map meets the 8.33 ms frame budget.

The Actions scenarios cover named-container image edits, environment preservation and add/rename/remove, quantities with distinct request and limit units, metadata key preservation, rollback revision selection, and guarded review before execution. YAML scenarios cover shape and encoding changes and content-preserving formatting in YAML and KYAML, including discovered resources read through their details command. The fixture includes multiple containers and referenced environment values so replacing a whole container or environment list fails the exact request check.

## Native gaps

Local preferences now persist through Backend in a versioned native document under [ADR 0030](../../docs/decisions/0030-gpui-local-preferences.md). The fixture backend stays in memory. The 19 rows previously disabled for missing browser storage now accept edits. Footer and delivery view choices also persist. Saved local workspaces, imported workspace documents and workspace-scoped RBAC review notes survive restart. Corruption and failed writes remain visible and preserve existing files. Remote workspace, profile and review data stays session-only pending ADR 0023.

Argo CD supports session profiles with external HTTPS token authentication, explicit connection selection and disconnect, connected Application inspection and comparison, guarded Refresh, Sync and rollback, read-only YAML editors, and highlighted diffs. The shared `argo-connected`, `argo-desired`, and `argo-review` scenarios exercise these paths. Native-only tests cover failed and cancelled connections, stale Application and profile reads, and failed execution consuming its confirmation.

Argo CD still lacks persisted profiles, remembered credentials, discovered Service tunnels and their target confirmation, local username/password authentication, custom CAs and insecure TLS controls, selectable rollback history and advanced Sync options, connected inspection in the managed-resource workspace, and background operation outcome observation. The native connection form offers external HTTPS and token authentication only. Rollback reviews the previous history entry, Sync inherits automated prune, and an accepted or uncertain submission blocks another submission until inspection is reloaded. Acceptance does not claim completion. These gaps remain native work rather than new backend command paths. ADR 0030 stores Argo profile metadata, but the connection form does not yet read or write it.

Saved Service forwards and the live-session defaults share this document with the other preferences. The separate session-preferences file writer is removed. Local migration preserves existing presets and the original file. Exec and guarded forward controls came from the live-session parity work.

Native gaps remain in workspace editing and multi-context reads; the connected Argo CD gaps listed above; automatic browser preference import; native update installation; MCP settings; browser opening; frontend latency collection and DOM-only diagnostics; and platform packaging. Preferences for unsupported native features are stored for when those features are available.

The workspace scenarios check note identity, workspace isolation and restart, clearing notes and preserving risk flags. Preferences scenarios check settings after recreating the client and check YAML defaults in the editor. Production kubeconfig writes use the desktop settings path initialized by the entry point. Native dialogs, remote transport and update-feed checks have controlled integration coverage; GPUI installation remains disabled.

This list records the delivery boundary so far. It is not an exhaustive inventory of every interaction in those areas. Full parity requires acceptance scenarios for the complete product surface.
