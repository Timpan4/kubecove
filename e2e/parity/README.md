# Frontend feature parity

The Svelte web frontend and the GPUI native client share one set of UI acceptance tests. Each file in `scenarios/` declares scenarios, and both drivers run every scenario against the same data:

- `web.ts` drives the Svelte app in Chromium with the browser mocks from `src/lib/tauri-dev-mocks.ts`.
- `src-tauri/gpui/tests/parity.rs` drives the GPUI client offscreen with `visual/export-fixture.ts`'s export of those same mock responses, through `src-tauri/gpui/tests/common/`.

`bun run e2e:parity` exports the fixture, runs both drivers and passes only when both report every scenario as passed. A passing mock scenario proves UI integration through the command seam, not a real Kubernetes request or a packaged application.

## Focused checks

Run new scenarios in each driver before the full suite. This catches missing native test registration and driver input mistakes without rerunning unrelated views. From the worktree root, with its local TMPDIR already set:

1. Run a web scenario prefix. This example checks the kind picker and records screenshots and `web.json`.

   ```sh
   mkdir -p e2e/artifacts/parity-focus
   PARITY_PREFIX=kind-picker nice -n 10 bun -e 'import { loadContract } from "./e2e/parity/contract"; import { runWeb } from "./e2e/parity/web"; const contract = await loadContract(); await runWeb({ scenarios: contract.scenarios.filter((scenario) => scenario.id.startsWith(process.env.PARITY_PREFIX ?? "")) }, `${process.cwd()}/e2e/artifacts/parity-focus`, new AbortController().signal);'
   ```

2. Export the same fixture and run that prefix offscreen in GPUI. The native input helper sends Backspace for an empty replacement; typing an empty string does not clear an input.

   ```sh
   bun e2e/parity/visual/export-fixture.ts e2e/artifacts/parity-focus/fixture.json
   KUBECOVE_PARITY_FIXTURE="$PWD/e2e/artifacts/parity-focus/fixture.json" \
   KUBECOVE_E2E_ARTIFACTS="$PWD/e2e/artifacts/parity-focus" \
   KUBECOVE_PARITY_SCENARIO_PREFIX=kind-picker \
   CARGO_TARGET_DIR="$PWD/src-tauri/gpui/target" \
   CARGO_BUILD_BUILD_DIR="$PWD/src-tauri/gpui/build" \
   CARGO_BUILD_JOBS=1 nice -n 10 \
   cargo test --manifest-path src-tauri/gpui/Cargo.toml --test parity
   ```

3. Run `bun run e2e:parity` without the prefix filter for the completion gate. Focused reports prove only the scenarios they name.

## Equal-tests rule

A UI change in either client lands with a shared scenario that both drivers pass. `contract.test.ts` checks two invariants on every `bun test` run:

1. Every view in `visual/views.ts` has at least one scenario whose ID starts with `<view>.`. Adding a native view to the visual harness without a shared scenario fails the check.
2. Every action in `ACTIONS` (`contract.ts`) has a handler in both `web.ts` and the modules under `tests/parity/`, dispatched by `parity.rs`.

An action names user-visible behavior, not a selector. Each driver maps it to its own UI: GPUI test IDs and labels, and web roles, names and text. When an action cannot be expressed in both clients, the clients have diverged and the divergence is the bug to fix.

Native-only tests in `src-tauri/gpui/tests/native.rs` and `src-tauri/gpui/tests/native/` cover what the browser mocks cannot express: read errors, cancellation, stale asynchronous results after scope changes, sessions from other sources, dirty drafts that block closing, refused log streams and dry runs, failed guarded executions, log streams that outlive their detail, dry runs of a draft that changed before they finished, and kubeconfig settings that the native backend cannot save. Native preference tests additionally cover file-backed process restart, migration, corrupt or future documents, stale revisions and failed writes. Their paths stay in worktree-local temporary directories.

`live-data.json` replays the same scripted watch messages in both drivers. It covers resource identity, deletion, reconnect catch-up, stale read failures, permission-limited polling, scope changes, event order, and discovered custom kinds in the sidebar and picker. Native stream tests also check cancellation while opening, dropped streams, and receivers that outlive their page, detail or context. The native client reuses the typed Rust watch commands, the web's 15,000 ms recovery interval from `resource-watch.ts`, and its 250 ms refresh throttle from `live-view-refresh.ts`. Pending row patches apply at rendering without rebuilding the table's filtered rows.

The ignored `watch_burst_frame_time` native test measures a burst against the 10,000-row fixture from `visual/export-watch-burst.ts`. It reports offscreen debug CPU frame construction and executor drain time, not GPU presentation or a release frame rate. The measured frames exceed the 8.33 ms requirement. Live-data scenarios do not prove 120 fps.

The detail-tab scenarios run guarded operations only against the browser mocks. The fixture backend answers a preview or execution only when its request matches the exported one exactly, so a wrong target or a missing guard field fails the scenario instead of reaching a cluster.

The `coverage-*.json` scenarios cover workspace deletion and import collisions, palette keyboard and dismissal controls, table page and group totals, map viewport and scope controls, timeline and event searches, log layouts and stream controls, force-conflict review and diff styles, action drafts, exec and forward controls, retained sessions, Helm resource entry points, RBAC form fields and evidence, incident context and failure states, and failed kubeconfig reads and saves. Pending, partial, empty and failed reads come from `tauri-dev-mock-coverage.ts`; the native fixture exports those same responses. Log viewport scenarios request the production default tail of 200 lines, then inject a subsequent line to check Jump to latest. Clipboard checks use page-local and native in-memory substitutes.

The shared YAML editor scenario covers selection replacement and document Home/End keys. Ctrl+Z undoes an edit in GPUI and leaves the web editor unchanged. The web editor scrolls in the detail pane; native code configures a fixed-height editor with internal scrolling. Folding and syntax/indent decoration equivalence remain unverified. Remote kubeconfig editing also needs a product decision: the native remote backend disables server-owned edits, while the web exposes server-path and environment-variable edits.

## Failure modes

[Scripted backend timing](mock-timing.md) records the asynchronous failure modes,
release-gate contract, source-derived retry window and current model boundaries.

Write acceptance scenarios before implementing the behavior they cover. The current scenarios target wrong-resource selection when kinds share a name, missing YAML identity, a detail panel that cannot close, an empty search that retains old rows, refresh that resets the search, event sort order and type filters, GitOps counts, cluster-scoped listings, an incident guide that ignores the signal filter, port forwards hidden by the source filter, aggregated log lines that lose their order, log filters and level toggles that hide the wrong lines, a log stream that survives a resource switch, a revision diff against the wrong comparison, a review that shows the wrong target or diff, a delete that executes without the typed name, a scale review with the wrong replica count, a dry run that skips the server, YAML apply offered for a Secret, RBAC risks that are not inherited from bindings and roles, a risk filter that keeps the wrong objects, an inspector or access check for the wrong subject, a workspace that opens or reopens with the wrong context or namespaces, kubeconfig settings that do not survive leaving Settings, settings search that keeps non-matching categories, and palette results or targets that ignore the query.

Keep report files under `e2e/artifacts/`. Each report names the driver and scenario IDs it actually ran. Missing evidence is a failure.

The topology-metrics scenarios cover namespace-scoped Network Flow edges, switching between Network Flow and Ownership, node selection, an empty graph, resource metric identity and units, missing samples, and the app process usage footer. Resource metrics render in the resource-page availability card, CPU and Memory table columns, and map-node hints. Resource detail matches Svelte and has no metrics section. The footer reads app process usage separately and its setting persists in the native local preference document. The visual harness captures `network-flow`, `metrics`, and `usage-footer`; passing screenshots and mock scenarios do not prove the 4,000-node map meets the 8.33 ms frame budget.

The Actions scenarios cover named-container image edits, environment preservation and add/rename/remove, quantities with distinct request and limit units, metadata key preservation, rollback revision selection, and guarded review before execution. YAML scenarios cover shape and encoding changes and content-preserving formatting in YAML and KYAML, including discovered resources read through their details command. The fixture includes multiple containers and referenced environment values so replacing a whole container or environment list fails the exact request check.

## Native gaps

The native custom-resource sidebar now searches discovered kinds by kind, plural, short name, API group and API version, using the web's trimmed case-insensitive match. The kind picker supports independent checkboxes and Select all kinds. Background reads preserve the chosen kinds and resource search; namespace navigation restores the destination scope's defaults. `sidebar-live.json` covers empty discovered kinds, changing and clearing search, multiple selections, unchecked rows after refresh and namespace changes.

GitOps now refreshes from Application, ApplicationSet, AppProject and discovered Flux watches. It reuses the web's 250 ms coalescing delay from `src/lib/live-view-refresh.ts` and 15,000 ms watch recovery interval from `src/lib/resource-watch.ts`. Helm, RBAC and Incidents refetch stale reads on page entry or window focus. Their staleness values come from `HelmSurface.svelte` (15,000 ms lists/details, 30,000 ms reconciliation), `RbacSurface.svelte` (30,000 ms) and `IncidentSurface.svelte` (15,000 ms). Those pages have no web polling interval. RBAC and Incidents also retain explicit Refresh. `page-live-reads.json` covers watch/focus updates and failed refreshes that preserve cached rows and selection. Native page tests cover cancelled reads on navigation and context changes, failed refresh recovery and a watch change during an in-flight read. These checks use mock commands and offscreen windows.

The exec terminal has shared scenarios for drag copy across hard lines, double-click words with wide and combining characters, selection after output and scrolling, paste through guarded stdin, tab-local selection, copying stopped output, and completed-line reflow on resize. Native parser tests cover history eviction, hard versus soft line breaks, explicit spaces, alternate-screen history, cursor placement and split UTF-8 input. Clipboard checks use browser copy/paste event data and GPUI's in-memory test clipboard. They never read or write the system clipboard. The web font setting now resolves its CSS variable before xterm uses canvas measurement.

Native rendering caches only the viewport. Reflow walks retained history only on column changes, and resize requests run serially with the latest pending dimensions. The 8.33 ms frame budget has not been measured for this change. Terminal differences remain in selection autoscroll outside the viewport, rectangular and triple-click selection, alternate-click cursor movement, native context menus and primary-selection paste, remote cursor bar/underline shapes and inactive cursor outlines. Both clients forward SGR mouse reports; the web subscribes only to xterm's onData, so its legacy onBinary mouse reports do not reach stdin. Mock parity does not prove platform clipboard integration or a live remote terminal application.

Local preferences now persist through Backend in a versioned native document under [ADR 0030](../../docs/decisions/0030-gpui-local-preferences.md). The fixture backend stays in memory. The 19 rows previously disabled for missing browser storage now accept edits. Footer and delivery view choices also persist. Saved local workspaces, imported workspace documents and workspace-scoped RBAC review notes survive restart. Corruption and failed writes remain visible and preserve existing files. Remote workspace, profile and review data stays session-only pending ADR 0023.

Argo CD profile metadata and workspace connection preferences now use the ADR 0030 store. Remembered credentials stay in the existing Rust credential adapters. Tokens, passwords, custom CAs and insecure TLS choices never enter the preference document. Remote metadata remains session-only.

The connection form supports external HTTPS, discovered Service tunnels with exact Pod and certificate confirmation, token or local login, session custom CAs and an explicit insecure TLS warning. Saved profiles reconnect through the credential adapter. Inspection and comparison use the selected connection in both the detail and managed-resource workspace. Rollback uses selectable history. Advanced Sync reviews revision, prune, dry-run and force options after Application-name confirmation.

The shared `argo-completion.json` scenarios cover saved profiles and preferences after client recreation, tunnel confirmation, local login and session TLS, advanced Sync, selected rollback history, connected workspace inspection and background completion after the detail closes. Existing `argo-connected`, `argo-desired` and `argo-review` scenarios retain their coverage. Native tests cover refused and cancelled target preview, connection and inspection failures, stale results, failed submissions, observed operation failure, replacement of the Application UID and unavailable outcome reads. Accepted submission remains distinct from completion. Outcome observation belongs to the workspace and continues after the detail closes, using the web's 15,000 ms recovery interval.

The native connection form opens a scrollable dialog from resource detail or the GitOps toolbar. Forget disables saved auto-reconnect before deleting the credential through the existing Rust adapter, then removes profile metadata. Failed stages remain visible and retryable. Discovery Retry and Confirm target again obtain fresh discovery and tunnel previews. Refresh and Hard refresh retain separate reviewed requests. Failed preflights and snapshots have explicit retry controls; accepted-state refresh retries observation without resubmitting. Connected inspection can fetch the focused resource comparison when the Application snapshot omits comparisons, with cancellation on selection or detail changes.

Refresh and Sync use split buttons with pending indicators and anchored option panels. Horizontal resource navigation keeps the detail's vertical position. Sync still keeps its Application-name confirmation inside the native options panel. Web focus trapping, outside-click dismissal, collision avoidance, editor internals and some scroll geometry differ. Mock parity and offscreen captures do not prove live authentication, keyring access, tunnel TLS or platform rendering.

`argo-gaps.json` adds coverage for Forget across restart, discovery and target recovery, distinct refresh actions, operation and snapshot retries, lazy comparison, provider detection errors, inventory failure and empty states, source and revision copy, managed workspace route copy, and removal and full-diff disclosure. The resource-focus scenario also covers a web bug where deep-proxy identity repeatedly reset the selected resource; the selected snapshot reference now uses raw Svelte state. Native tests cover partial Forget failures, comparison refusal, cancellation, stale resource selection and pending observations retained across state-refresh retry. Argo fixtures and their shared command argument types live in `src/lib/tauri-dev-mock-argo.ts` and `src/lib/tauri-dev-mock-contract.ts` so the mock command router stays within the TypeScript file cap.

Saved Service forwards and the live-session defaults share this document with the other preferences. The separate session-preferences file writer is removed. Local migration preserves existing presets and the original file. Exec and guarded forward controls came from the live-session parity work.

Secret YAML and detail reads remain backend-redacted. Turning off Redact secrets enables per-key Reveal and Hide controls in the YAML tab, matching Svelte. Reveals are detail-local, expire after the web's 60 seconds, and clear on tab, target, preference and scope changes. Pending reveals cancel when the detail closes. No revealed value enters preferences or diagnostics.

The native YAML error lens marks the parser's first invalid line and displays its message beside that line. Fixing the document or disabling YAML error lens clears it. Native diagnostics collect in-memory command round trips while Enable diagnostics is on. Settings -> Diagnostics supports Refresh, Clear and Copy, and reads shared backend timings and cache counters. Native timings measure command completion, including executor waiting, rather than GPU presentation. Copy includes only static command names, durations and success states. The collector uses the web's 500-event retention limit.

The documentation control calls a fixed-URL native opener behind Backend. Fixture backends record the wiki URL without opening a browser. Record interactions and Interaction report remain disabled because the web recorder measures DOM changes, browser frame intervals and input-to-paint. GPUI has no equivalent presentation-completion instrumentation at this seam. The Svelte-only Topology spike also remains disabled.

AI agents settings now reuse the desktop MCP host through Backend under [ADR 0028](../../docs/decisions/0028-read-only-mcp-server.md). Both clients read the same `mcp-settings.json` and `mcp-token` in the desktop config directory. The MCP host stays local while a remote runtime is selected. The native client can enable or stop the loopback listener, save its port, copy the token or Claude Code command, and rotate the token. Tokens never display or enter the ADR 0030 preferences document. Command copy stays disabled until the listener has a URL; invalid or unchanged port drafts cannot be saved. Pending actions disable conflicting controls. Load, save, token and bind failures remain visible.

The shared `settings-agents.*` scenarios cover defaults, port validation and reopening, listener changes, copy-only credentials, rotation and local ownership during remote selection. Their fixture records requests without binding a port. Native tests cover occupied ports, token-file startup failure, token rotation against the read-only endpoint, enabled startup after backend recreation, corrupt or unreadable settings, failed saves and private token-file permissions. All storage paths belong to the worktree, and live listener tests choose and close their own loopback port.

Workspace editing supports rename, primary context, group membership and namespaces while preserving the record ID, kinds, retained import fields and eligible reviews. The shared workspace-editor and workspaces scenarios cover cancel, editing a different saved record, rename without duplication, restart, imported field preservation, Overview context and namespace totals, context removal and partial read failure. Native tests check cancellation of an old Overview read when the workspace changes. Overview aggregates the selected workspace contexts. Resources reads only the primary context in both clients, including details, YAML and guarded operations. Whether Resources should aggregate group members remains an open product question.

Native gaps remain in automatic browser preference import; native update installation; DOM-only diagnostics; and platform packaging. The YAML lens reports the first parser error rather than the web parser's complete warnings and errors. Latency reports refresh on request and do not claim render counters, input-to-paint timing or frame-rate measurements. Preferences for unsupported native features are stored for when those features are available.

Shell parity is incomplete. Overview operation buttons, incident-filter handoffs and incident Review in Actions have shared scenarios. Launcher read retries, empty context inventory and error-detail copy have shared recovery scenarios. `overview-completion.json` adds saved scope and restore warnings, scoped resource retry, shortcut preferences, context and namespace Compare cards, pin restart/isolation/unpin, namespace and Application visits, and pointer resource-group ordering. Remote settings scenarios assert the header host, server version and session grace. Pins and recents use the ADR 0030 preference store. Argo shortcuts and recent entries open GitOps but do not yet select the target Application. Imported entry metadata, removal reconciliation and full ordering coverage remain incomplete. The shared feedback interface stays available for other surfaces.

`sidebar-navigation.json` covers nested built-in kind and namespace trees, namespace-present custom kinds, separate chevron and row selection, and Collapse all. Collapsed parents do not construct descendants. Namespace Storage preserves its namespace filter while including cluster-scoped kinds. Built-in namespace groups remain available when Show Custom Resources is disabled. Selecting Cluster Overview clears a previous leaf-kind filter in both clients.

Remaining shell gaps include the compact navigation drawer, the namespace table, tree keyboard navigation, the foreground loading bar, the live-session header popover, and common health and timestamp tooltips. Navigation restoration must remain session-only. `src/lib/path-state.ts` reads the web URL hash and sessionStorage; native navigation snapshots must not enter the preference document. Native session navigation restore and header update-status interactions still need implementation and shared scenarios. Resource-group ordering supports keyboard movement and native pointer drag, including persistence. Update installation stays disabled by user decision.

Overview restore warnings still depend on a successful primary resource load. Missing primary context and missing-kind recovery need shared scenarios and independent availability state. Editing generated Compare shortcuts must retain their embedded comparison metadata before shortcut parity is complete.

The workspace scenarios check note identity, workspace isolation and restart, clearing notes and preserving risk flags. Preferences scenarios check settings after recreating the client and check YAML defaults in the editor. Production kubeconfig writes use the desktop settings path initialized by the entry point. Native dialogs, remote transport and update-feed checks have controlled integration coverage; GPUI installation remains disabled.

This list records the delivery boundary so far. It is not an exhaustive inventory of every interaction in those areas. Full parity requires acceptance scenarios for the complete product surface.
