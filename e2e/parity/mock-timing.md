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

The retry observation window is 30,000 ms. Its source is
the installed `@tanstack/query-core/src/retryer.ts`, `defaultRetryDelay`, which
caps backoff at that duration. Both drivers advance a test clock for this check.
After watch release the native driver advances the existing 250 ms refresh
throttle from `src/lib/live-view-refresh.ts`, as other watch scenarios do.
The existing browser mock's 35 ms delay is unchanged and is not measurement evidence.

## Coverage boundary

The browser interceptor covers command reads. Native fixture gates currently cover
resource scope, discovery, ownership topology and resource-watch opening. Native request IDs are not IPC
arguments at its Backend trait, so cancellation there is expressed by dropping a
read. Neither model reproduces completed-cache freshness or cache eviction.

No expected-failure mechanism exists in the shared report contract. Known native
failures remain failures and make the full parity check fail.

## Focused results

The focused web run passes all six scenarios. The native runs pass
`mock-timing.late-topology`, `mock-timing.credentials`, and
`mock-timing.context-switch`.

Native failures remain in `mock-timing.cancelled-topology` (the recovered node
never appears), `mock-timing.forbidden` (the no-retry count passes but the map
error is absent), and `mock-timing.list-watch-gap` (the row remains Healthy
instead of Degraded after watch opening and the refresh throttle). These are
failed acceptance assertions, not expected-failure skips. Native production
code is unchanged. No web product bug was proven in these focused scenarios.

Repeatable commands and evidence are under `e2e/artifacts/mock-timing/` in this
worktree. `web.json` records the six web results. Each `native-<case>/run.log`
records the native run; successful native cases also emit `gpui.json`.

The full `bun run e2e:parity` run records 163 passing web scenarios in
`e2e/artifacts/parity-fb84cd09-ce6c-4f42-bdbe-9b28e0ffbf4c/web.json`.
The native runner stops at `mock-timing.cancelled-topology`; its progress and
panic are retained in that directory. It does not emit a complete `gpui.json`
after a panic, so the full runner also reports the missing native report.

Typecheck, Svelte check, Biome lint, contract and stylesheet tests, docs check,
diff whitespace check, Rust formatting and Clippy pass. Native library tests
pass 20 tests; the native integration suite passes 72 with one existing ignored
test. Anti-slop exits unsuccessfully both before and after: 75 errors and one
warning, with no new diagnostic messages. Biome formatting is disabled by the
repository configuration. No native production changes, push or PR accompany
this work. The changes remain uncommitted because the required parity check fails.

The model excludes real authentication, network transport, completed-cache
freshness, cache eviction, and platform rendering. Permanent error scripting
accepts backend error kinds, but the shared denial scenario currently exercises
`forbidden`, not every deterministic error kind.
