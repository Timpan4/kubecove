# Frontend memory characterization

Issue [#417](https://github.com/Timpan4/kubecove/issues/417) measured resource payloads, projections, topology, metrics, YAML, and inactive queries under the former 90-second retention policy. Owner decision [#418](https://github.com/Timpan4/kubecove/issues/418) extends inactive resource, topology, and metrics retention to 300 seconds in [#493](https://github.com/Timpan4/kubecove/issues/493). Payload contracts, visible counts, sorting, filtering, and selection stay unchanged. The historical measurements below remain observations of the 90-second policy.

## Failure modes

These failure modes were recorded before implementing the measurement helpers:

- Query keys or payload contents leak into reports instead of fixed root classes and counts.
- Serialized weight counts UTF-16 code units instead of UTF-8 bytes.
- Unavailable heap inspection or unserializable payloads appear as zero.
- Query counts omit empty queries, confuse enabled and disabled observers, or count empty collections as one item.
- Instrumentation changes collection defaults or retains payloads itself.
- Collection-wait polling serializes large payloads and distorts memory observations.
- Search/sort or topology probes keep temporary results across collection checkpoints.
- Synthetic workloads appear as live-cluster or complete heap attribution.
- Heap sampling includes temporary allocations from the payload-weight probe.

Focused tests cover UTF-8 sizing, redaction, unknown-root grouping, disabled observers, empty collections, unserializable payloads, and counts-only polling. Native profiles exercise the existing typed Rust commands against a GET-only HTTPS loopback fixture. The fixture certificate and private key stay in the owned temporary run directory and are removed during cleanup. The normal application security boundary is unchanged.

## Reproduction and interpretation

Run `bun run perf:frontend` for the existing deterministic measurements plus normal and large memory fixtures. To isolate the new workload from the other fixtures in that script, run:

```sh
bun -e 'import { characterizeMemoryFixtures } from "./scripts/perf-memory-fixtures"; console.log(JSON.stringify(characterizeMemoryFixtures(), null, 2))'
bun run perf:startup --memory-fixture normal
bun run perf:startup --memory-fixture large
```

The optional native fixture requires the OpenSSL CLI on PATH, including on Windows. It generates an ephemeral loopback server certificate without changing the host trust store. Native profiling also requires the existing desktop dependencies and a working display. This host uses headless Weston with the Pixman renderer. No Kubernetes cluster is deployed or modified.

The deterministic normal fixture uses the existing mixed browser mock set of 65 resources. Its large fixture repeats those summaries as 10,000 Pod rows, matching the existing metrics workload size. The native fixtures instead generate 65 or 10,000 synthetic Pods, plus one Deployment, ReplicaSet, Service, Node, and Namespace. They share counts, but their shapes and payload sizes differ.

`frontend-memory.json` is written under each native run's `e2e/artifacts/desktop-*/` directory after every stage. It records source SHA and dirty state. A dirty run describes the working tree, not just that commit. Heap bytes are sampled before payload serialization. Counts-only polling avoids JSON allocation during the real configured collection interval. An enabled observer makes a query active; a disabled observer can still keep an inactive query observed. The report records both distinctions.

Payload bytes are UTF-8 JSON estimates, not allocation sizes. Source, merged, projection, and topology estimates overlap because objects share references; do not add them. Bun's JavaScriptCore heap samples use explicit collection. WebView collection is not forced. Process RSS includes shared pages and allocator high-water marks. DOM events exercise application handlers; physical pointer input is unverified. Timings include driver, query probing, and process sampling overhead. These are single local observations, not cross-platform distributions.

## Deterministic fixture observations

The isolated Bun run produced these counts and payload estimates:

| Measurement | Mixed normal fixture | Large fixture |
| --- | ---: | ---: |
| Source rows | 65 | 10,000 |
| Source UTF-8 JSON bytes | 47,893 | 7,289,291 |
| Search index entries | 65 | 10,000 |
| Display rows | 65 | 10,000 |
| Page rows | 50 | 50 |
| Render model entries | 68 | 54 |
| Source topology nodes | 65 | 10,000 |
| Derived flow nodes with existing grouping | 15 | 1 |
| Metrics JSON bytes | 9,558 | 1,417,034 |
| Merged row JSON bytes | 45,960 | 6,954,917 |
| Projection JSON bytes | 54,030 | 8,075,359 |
| Topology JSON bytes | 54,743 | 8,252,732 |

The six existing-state search/sort changes preserved source counts. The no-match search produced zero rows. The large fixture's normal name-sort steps took 34.30, 18.37, and 17.43 ms; its metrics merge took 6.83 ms, projection 19.55 ms, and grouped topology construction 66.29 ms. Grouping collapsed 10,000 source nodes to one derived flow node, so this does not characterize rendering 10,000 visible nodes.

After explicit Bun collection, the normal table sample held 1,751,849 heap bytes and the large table 10,507,363. Large topology open and close samples held 12,141,178 and 12,024,095 heap bytes. The corresponding topology estimate shares summaries with the source table. These values describe the entire reachable fixture context, not isolated ownership-map allocations. YAML samples used one generated resource, 134 or 135 UTF-8 bytes, and do not represent large raw manifests.

The existing inactive-query fixture retains 25,000 entries for each of three roots. Immediately before collection, `resources`, `resource-metrics`, and `resource-topology` weighed 3,277,781, 3,452,781, and 3,477,781 JSON bytes. All three queries existed at 89,999 ms and were absent at 90,000 ms. Explicit Bun collection reduced the process's reported JS heap from 58,765,109 to 42,409,529 bytes. RSS remained 270,041,088 bytes. Releasing queries therefore does not imply an immediate equivalent RSS reduction.

## Native WebKitGTK observations

Both runs passed in the same app session across launcher, rows, populated metrics, detail, YAML, topology open/close, launcher switching, warm navigation, the actual collection interval, and refetch. Source SHA was `efddb7b73bcca3fc80687ac31afadbe1dcf43b72` with `sourceDirty: true`; these are pre-commit working-tree measurements. Artifacts are `desktop-1790720161978-1117722-ad281301` (normal) and `desktop-1790720474267-1127098-1bf76824` (large).

| Operation or count | Native normal | Native large |
| --- | ---: | ---: |
| First resource rows, ms | 344.77 | 15563.75 |
| Detail open, ms | 456.47 | 10146.42 |
| YAML open, ms | 54.88 | 134.16 |
| Topology reopen, ms | 95.20 | 19271.58 |
| Topology close, ms | 90.35 | 3227.55 |
| Warm back navigation, ms | 190.78 | 10829.18 |
| Rows after collection/refetch, ms | 274.74 | 17534.24 |

| Cached root before collection | Native normal JSON bytes | Native large JSON bytes |
| --- | ---: | ---: |
| resources | 49,205 | 7,300,815 |
| resource-metrics | 16,148 | 2,447,325 |
| resource-topology | 93,000 | 13,937,573 |

| Process sample, resident bytes | Native normal | Native large |
| --- | ---: | ---: |
| Launcher host | 230,838,272 | 231,186,432 |
| Launcher WebView | 488,660,992 | 486,625,280 |
| Inactive host before collection | 243,843,072 | 994,717,696 |
| Inactive WebView before collection | 857,423,872 | 7,086,960,640 |
| Host after collection | 241,025,024 | 994,488,320 |
| WebView after collection | 829,485,056 | 1,749,704,704 |
| WebView after refetch | 859,762,688 | 6,547,075,072 |

The root item counts before collection were resources 68/10,003, metrics 67/10,002, and topology 134/20,004. Aggregate topology counts include nodes and edges; aggregate metrics counts include known collection fields. Both resource tables contained 50 row actions in the DOM. They were not necessarily in the viewport. Existing artifacts call that field `visibleResourceActions`; the collector now names it `renderedResourceActions`. Derived flow-node counts vary during layout transitions, and the collection report does not claim a stable painted graph. Captured screenshots were 800 by 553 pixels; the collector now also records the CSS viewport directly.

Before collection, every large root existed and had only inactive, unobserved queries. There were three `resources` queries and one each for metrics and topology. After collection, those five queries were absent; other query families remained, bringing the total from 26 to 21. Returning restored the query families and rows. Query defaults and backend cache behavior were unchanged.

WebKitGTK did not expose `performance.memory`, and the embedded WebDriver exposes no browser heap snapshot interface here. Native JS heap and retained-object heap attribution are unavailable, not zero. Query-cache observations identify the retained `Query.state.data` paths by root and estimate their serialization weight; they do not explain the whole WebView RSS. In particular, the large workload reached 9,089,306,624 WebView resident bytes during warm navigation, far above its JSON estimates. That is an observed process sample, not proof that query retention caused those bytes or that this run isolated a leak. Closing topology retained the raw topology query while removing rendered flow elements.

## Browser heap observations

The focused Chrome/WDIO spec uses CDP `Runtime.queryObjects(Query.prototype)` against the exact already-loaded query module URL. Omitting Vite's version query creates a different constructor identity and returns an empty object set; the spec rejects that result and requires populated resource Query data. CDP references are released after every observation. The normal browser build has no memory-profile global.

In `e2e/artifacts/browser-memory-target-green/browser-query-heap.json`, both resource-open and inactive-launcher checkpoints retained five resource Query objects containing 112 array entries and 82,973 JSON bytes across their scope results, and two topology Query objects containing 154,304 JSON bytes. These are repeated result entries, not 112 unique resources. The metric Query object existed but its payload was not populated at these early checkpoints. Native runs separately measured populated metrics. The inspector observed 28 Query objects in total, including other families and objects that may be retained outside the cache.

Chrome's JS heap before the two inspections was 64,077,652 and 53,867,100 bytes. Backing storage and embedder heap are separate fields in the artifact. CDP inspection can collect garbage; serialized Query payload weights are not heap ownership sizes, and these browser mocks do not establish native WebKit allocation attribution. The retained object path is `Query.state.data`; resource identifiers and query-key suffixes never enter the report. Run `bun run e2e:fast` to reproduce this browser check alongside the existing scenarios.

## Decision note for #418

The owner selected 300 seconds of inactive retention for `resources`, `resource-topology`, and `resource-metrics` so operators can return to views during work. The interval starts when a query loses its last observer. Reopening cancels collection; leaving again starts a new five-minute window. Existing freshness, refetch, and watch behavior remains in place. Revealed secrets still use their explicit zero-retention settings. Backend cache budgets and the release profile are unchanged.

The choice keeps cached payloads reachable longer than the former 90-second policy. It does not establish a whole-WebView memory budget or attribute the process RSS increase to queries. The measured path is launcher to Resources, inspection, launcher switching, and return. These historical comparisons informed the decision:

| Choice | Measured memory or retained path | Refetch and UX cost |
| --- | --- | --- |
| Keep the existing 90-second inactive retention | Keeps the three measured root families reachable during back navigation. Both native runs collected them at the configured interval. | Warm return took 190.78 ms normal and 10,829.18 ms large. It preserves the current back-navigation contract, but large-workload rendering remains expensive even with cached payloads. |
| Collect inactive resource/topology/metrics queries earlier | Targets `Query.state.data` for the measured 49,205/7,300,815-byte resource, 93,000/13,937,573-byte topology, and 16,148/2,447,325-byte metrics JSON estimates. These are overlapping serialized views, not additive heap savings. | Returning after collection took 274.74 ms normal and 17,534.24 ms large. The operator waits for rows again; live-cluster network cost is unmeasured. A new interval needs an owner decision. |
| Retain different root families differently | The native large topology estimate is larger than resources or metrics. Closing the map removes rendered flow elements while its raw query remains reachable. | Topology reopening took 95.20 ms normal and 19,271.58 ms large. Keeping resource rows warm while collecting topology changes map-return behavior and requires a separate accepted contract. |
| Reduce derived work without changing retention | The deterministic large projection has 10,000 search entries and an 8,075,359-byte JSON representation, while the native page holds 50 row actions and can rebuild much larger topology source data. | Existing projection, repeated sort, and topology construction durations identify work to investigate. The probe has no whole-WebView heap attribution, so it cannot promise an RSS saving or justify a payload-contract change. |

The table describes the former policy and alternatives; it does not measure the new five-minute interval. Shorter or different per-root retention and projection changes are not part of the selected implementation. Any recommendation to shorten retention should cite the specific root family and observed refetch cost. Any recommendation to change topology or table projections should first isolate the measured derived path rather than assign the entire process increase to cached JSON.

## Verification

The failure-mode tests, deterministic performance script, native normal and large scenarios, TypeScript, Svelte, Biome, anti-slop lint, and documentation checks passed. Final-source reruns also passed all 12 stages in `desktop-1790721907510-1168464-5a76ef77` (normal) and `desktop-1790722045258-1172980-f9f7af82` (large). These reruns verify the YAML active-tab assertion and the renamed DOM count with a recorded 800 by 553 CSS-pixel viewport. The tables above retain the earlier observations. The normal frontend build excludes the profile hook identifier. Native screenshots confirm the fixture overview and generated YAML; offscreen DOM actions and graph counts do not prove physical pointer or painted-frame performance. macOS and Windows native measurements remain unmeasured.
