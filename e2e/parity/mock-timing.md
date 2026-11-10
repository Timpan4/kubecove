# Scripted backend timing

## Failure modes written before implementation

1. A dirty shared load restarts, earlier waiters receive `cancelled`, and the UI never retries.
2. An older response arrives after a newer response and replaces current data.
3. A context changes during a read and the previous context's rows appear in the destination.
4. First-use credential resolution holds all reads for a context, leaving the UI permanently loading or failed after release.
5. A forbidden resource triggers repeated automatic requests.
6. A watch event between the list snapshot and watch registration disappears, leaving stale rows.

## Source and scope

`src-tauri/crates/kube/src/commands/live_store.rs` defines clean-load joining,
dirty-load restart and cancellation of previous waiters. `cancellation.rs` defines
scope cancellation and replacement of a duplicate request ID. `finite_read.rs`
wraps cancellable finite reads. `src/lib/query-retry.ts` defines transient errors
and deterministic failures. These are separate mechanisms.

Timing controls use explicit release steps, with no simulated millisecond latency.
A held command remains pending until released. Different keys can finish in any
scripted order. Default development reads retain their existing behavior.

## Contract

The `mockTiming` action takes `control` with `command`, `context`, optional
`namespace`, and an operation:

1. `hold` holds subsequent reads. Leading hold actions run before workspace
   creation in both drivers so initial loads cannot escape the script.
2. `expectPending` waits for a matching held read in the browser, or drains the
   native executor and asserts it exists. This is the synchronization boundary.
3. `dirty` invalidates held loads. `replayLiveData` update and delete also
   invalidate loads for their context, even before a watch subscribes.
4. `contend` introduces another backend consumer of each held request. Clean
   consumers share the snapshot; dirty loads cancel earlier consumers and restart.
5. `release` completes matching reads. A namespace limits release to that scope.
6. `fail` rejects matching reads and later requests with the supplied backend
   error kind until `release`. `failOnce` rejects the held load but allows retry.
7. `expectRequests` checks an exact request count. `expectNoRetry` compares the
   count with its value at failure after advancing the retry observation window.

`credential` is a context gate rather than an IPC command. Holding it delays
context reads until release. It uses no real credential plugin or user settings.
Its source is `cluster_access/clients.rs`: construction takes
`client_build_gate(source_key, cluster_context)`, resolves configuration and
builds the client before releasing waiters. Finite construction races generation
cancellation through `resolve_before_cancel`.
The web mock also registers normalized `cancelScope` and `requestId` pairs,
cancels replaced IDs, and implements scope and workspace cancellation. Native
fixture reads use dropped futures as their cancellation boundary.

`setTopologyMode` accepts `pending: true` to start the interaction without waiting
for completion. Ordinary scenarios keep their existing wait.

## Watch start versions

Each client starts a resource watch at the list version of the rows its view
shows for that key, and omits the version when it has none
(`streams/watch.rs` then starts at `"0"`, which replays every object). Both
mocks model this with one list version: `"1"`, raised by one for each
`replayLiveData` update or delete. Scope and kind reads, and the Argo CD and
Flux lists of the GitOps page, report the version current when their rows were
read, so a held read keeps its version. `gitops.watch-start-version` checks the
GitOps watches.

`argo-workspace.watch-start-version` checks a focused Application's resource
watches. Its inspection reads the Application, not lists of the kinds it
manages, so those kinds keep the table's list version. The scenario turns off
"Show ownership map by default" first, because the open map would start them
without one. The GPUI fixture reports one version per target namespace list, as
`delivery_workspace::resources` does.

A watch start replays the scripted change only when its version is older than
the version that first showed the change, or when it has no version. The
`expectWatchStart` step checks the version the newest watch of a kind started
at, from `lastWatchStart` in `tauri-dev-mock-live.ts` and `expect_watch_start`
in `tests/common/live_data.rs`.

`mock-timing.watch-start-version` collapses the ownership map first. The web
map has no list version, so a key it shows starts without one.

That scenario adds a kind to a view that already watches others, so the web
starts the watch through `update` of the existing observer. The web creates the
observer, and starts a view's first watches, while the ownership map is open by
default, so those keys start without a version and a dropped version looks the
same. The kind picker has no control that empties the selection, which would
create a new observer after the map is collapsed. `live-view-refresh.test.ts`
covers that path instead: a new view's watch waits for its rows and starts at
their list version.

## Loading indicator timing

Both clients show a read's loading indicator only after the read has been
pending for 300 ms, and keep it for at least 300 ms once shown. The finished
data waits until then. `src/lib/loading-delay.svelte.ts` and
`src-tauri/gpui/src/inspector/loading.rs` hold the timing.

A scenario that checks an indicator runs `advanceClock` 300 ms before the check,
and again after the release before it checks the data. The native driver needs
both steps because its clock moves only when a step advances it. The web driver
installs its fake clock before load whenever a scenario uses `advanceClock`.
Time still runs there unless the scenario also uses `controlClock`.

The stale-data notice of the resource browser ("Showing last observed state.")
uses the same two times. The native driver advances its clock 300 ms before
every `expectLiveState` check, so a notice that is due has appeared and a
notice whose watches recovered has gone. `live-menu.notice-delay` checks both
times with the web clock paused. The native driver advances 250 ms after each
`replayLiveData`, but not once a scenario has run `controlClock`, because the
scenario then owns the clock in both drivers. That scenario checks the notice
is absent straight after the replay, present 300 ms later, still present
straight after recovery, and gone 1000 ms after that.

The `loading-delay.` scenarios pause the web clock with `controlClock` before the
resource table mounts, so both drivers measure the same intervals. The web mock
waits 35 ms before it reaches a hold, so these scenarios advance the clock before
`expectPending`.

The retry observation window is 30,000 ms. Its source is
the installed `@tanstack/query-core/src/retryer.ts`, `defaultRetryDelay`, which
caps backoff at that duration. Both drivers advance a test clock for this check.
After watch release the native driver advances the existing 250 ms refresh
throttle from `src/lib/live-view-refresh.ts`, as other watch scenarios do.
The existing browser mock's 35 ms delay is unchanged and is not measurement evidence.

## Map viewport transition timing

A map selection moves the viewport to the new fit over 120 ms, and a deselection
over 260 ms, the durations in `src/features/resources/OwnershipMapViewport.svelte`.
The GPUI client takes the time from the executor clock
(`src-tauri/gpui/src/inspector/map/motion.rs`), so in the native driver a
transition stays at its start until a step advances the clock.

`coverage-map.selection-pan` pauses the web clock with `controlClock` before it
selects a node, then advances both clocks and checks that the viewport holds,
moves, moves again and settles. Where the web driver waits for the map, as
Playwright does before it clicks a node, the native driver calls
`Inspector::finish_map_transition` instead of advancing its clock. Advancing
would also move loading delays and other timers, and the clock cannot advance
inside the window update where step handlers run, because advancing runs tasks.
`selectTopologyNode`, the `map-full` checks and the visual capture do this.

## Coverage boundary

The browser interceptor covers command reads. Native fixture gates currently cover
resource scope, discovery, ownership topology and resource-watch opening. Native request IDs are not IPC
arguments at its Backend trait, so cancellation there is expressed by dropping a
read. Neither model reproduces completed-cache freshness or cache eviction.

No expected-failure mechanism exists in the shared report contract. Known native
failures remain failures and make the full parity check fail.

## Current results

Both drivers pass all seven `mock-timing.` scenarios, checked with
`PARITY_PREFIX=mock-timing. bun run e2e:parity`. The earlier native failures in
`mock-timing.cancelled-topology`, `mock-timing.forbidden` and
`mock-timing.list-watch-gap` no longer reproduce, so those notes were stale.
Neither the fixture nor the app needed a change for them.

Each run writes `web.json`, `gpui.json` and `gpui.log` to its own
`e2e/artifacts/parity-<id>/` directory. The `mock-timing.`, `resources.` and
`overview.` prefixes all pass in both drivers on the current tree.

The model excludes real authentication, network transport, completed-cache
freshness, cache eviction, and platform rendering. Permanent error scripting
accepts backend error kinds, but the shared denial scenario currently exercises
`forbidden`, not every deterministic error kind.
