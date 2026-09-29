# Backend cache diagnostics

Issue [#419](https://github.com/Timpan4/kubecove/issues/419) adds redacted aggregate observations to the existing opt-in diagnostics path. Cache capacity, freshness, single-flight loading, cancellation, and dirty fallback remain unchanged.

Failure modes to verify before implementation:

- A hit, miss, coverage hit, or single-flight join is omitted or counted twice.
- A joined completion counts one eviction or restored reload failure more than once.
- Loading entries or their retained previous value disappear from state or weight counts.
- Cancellation or failed reload changes the previous value's dirty state.
- A diagnostic includes a cache key or an identifier from a value.
- Weight collection clones or serializes a payload under the cache lock.
- Diagnostics publish cache observations while disabled.
- Removing routine stderr changes an actionable error path.

Weight is a shallow payload lower bound, consisting of value headers and occupied collection elements. It excludes nested string/JSON allocations, allocator overhead, cache keys, hash-table overhead, and shared-future storage. Collection lengths are read without traversing, cloning, or serializing their values. The estimate is suitable for attributing payload classes, not for selecting a memory budget.

## Using the report

Enable Settings -> Diagnostics to inspect the backend cache table. Copy includes the same snapshots under `backend.caches`. The typed `get_backend_cache_diagnostics` command returns an empty array while diagnostics are disabled. Snapshot reads do not add timing events or displace latency history.

Counters accumulate since trace clear. A successful coverage peek counts as a hit. A new load or reload counts as a miss; a caller sharing an existing load counts as a join. Evictions count ready entries removed for capacity and entries removed by explicit refresh, including cancelled loading entries. A failed reload that restores a previous value counts once, even when callers share its completion. Clear resets counters and timing history without clearing cache values.

Ready and loading counts are disjoint. Dirty counts include the retained previous value during a reload. Item counts and bytes include retained previous values, excluding unavailable loading results. Cache limits, freshness, cancellation, and back-navigation behavior are unchanged.

## Repeatable measurements

`cargo run --release --manifest-path src-tauri/Cargo.toml --features bench-support --example cache_diagnostics` prints a redacted snapshot of cached Pod summaries and topology from the existing 500-app backend benchmark fixture. It runs no Kubernetes API calls. The example is excluded from default Cargo targets unless `bench-support` is enabled.

`bun run rust:bench -- inspect_cached_payload_weights_500_apps` measures snapshot collection against those retained payloads. Fixture generation runs outside the timed region. This measures local observation cost, not network latency or complete allocation size.

`bun run perf:startup` also exercises the real typed commands and Settings table in the native release-shaped app. Its `backend-cache-diagnostics.json` artifact verifies opt-in access, fixed fields, counter reset, and unchanged timing history. `backend-cache-diagnostics.png` records the table. The launcher fixture has no reachable cluster, so this native scenario does not measure populated production caches.

Local native verification passed on Linux x64, WebKitGTK 2.52.6, and Weston 14.0.2. The inspected 800px screenshot shows per-cache headings and readable Metric/Value tables. Native WebDriver DOM events exercise UI handlers because element commands stall on this headless Wayland host; physical pointer input and Windows/macOS rendering remain unverified locally.

## Release fixture results

The optimized Linux x64 example, built with Rust 1.98.1, retained these payload classes from the 500-app fixture:

| Cache label | Ready entries | Retained items | Shallow payload bytes |
| --- | ---: | ---: | ---: |
| `resources` | 1 | 500 Pod summaries | 304,024 |
| `topologies` | 1 | 2,000 nodes and 1,000 edges | 1,632,072 |
| Other four labels | 0 | 0 | 0 |

Each populated label recorded one miss and no hits, joins, evictions, or restored failures. Topology headers and occupied elements contribute more shallow retained weight than the Pod summary collection in this fixture. Nested strings and other excluded allocations may change their actual heap proportions. These observations support measuring topology payloads when considering issue [#420](https://github.com/Timpan4/kubecove/issues/420); they do not select a cache budget or retention policy.

The focused optimized wall-time benchmark measured a 129.8 ns median and 148.1 ns mean for collecting all six snapshots against the fixture. Its 100 samples and iterations use the installed Divan runner's defaults. Fixture initialization ran before the timed closure. This is one host run, not a performance target or a before/after improvement claim; CodSpeed provides the deterministic CI signal.
