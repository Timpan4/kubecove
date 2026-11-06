# Native frame measurement

Run from the repository root:

```sh
export CARGO_TARGET_DIR="$PWD/src-tauri/gpui/target"
export CARGO_BUILD_BUILD_DIR="$PWD/src-tauri/gpui/build"
export CARGO_BUILD_JOBS=1
mkdir -p .t
export TMPDIR="$PWD/.t"
nice -n 10 bun scripts/gpui-frames.ts --runs 3 --frames 240 --warmup 120
```

Each worktree must use its own build directories. Concurrent builds can overwrite path-crate artifacts in shared directories. Cargo builds the frame test with the release profile. The runner records compile-time build paths and the executable's SHA-256, then pins a copy in its artifact directory before running repetitions serially. It rejects build paths outside the current worktree and saved executables compiled in shared directories. Launch the runner with one `nice -n 10` adjustment. The runner adds no further adjustment and records each child's observed process priority beside its load averages. It forwards cancellation to its child and has no command timeout. Reports, raw nanosecond samples, build output, inputs and load averages remain in `e2e/artifacts/gpui-frames-*`. Remove `.t` after commands exit.

The default protocol is three runs of 240 measured samples after 120 warm-up samples per case. This provides a median and spread, and enough samples for nearest-rank p99. `--runs`, `--frames` and `--warmup` override the protocol. Positional case IDs can be separated by commas or spaces. `--binary` repeats a saved executable without rebuilding. `--manifest <Cargo.toml>` selects an archived source snapshot inside the current worktree. Both build directories must still be the current worktree's GPUI directories. Record the source commit and harness-only adapters when comparing an archived base. `--fixture <fixture.json>` copies an existing browser-mock export instead of generating another one; the deterministic watch workload is exported from that copy. Use the same input for both sides of a comparison. `--compare <report.json>` adds the full before/after table. A one-run diagnostic does not replace repeated acceptance measurements. A full run rejects duplicate case IDs and missing views from `e2e/parity/visual/views.ts`, so future view additions require frame coverage.

The runner records `uptime` before each repetition on Unix. On Linux it rejects live native parity processes and the parity runner before measurement and checks again when each case reports completion. If it observes overlap, it terminates only its own harness and preserves an invalid-run log. These snapshots supplement the shared heavy lock; every parity and frame command must honor that lock to prevent overlap between snapshots.

## What the numbers include

Measurement version 2 uses `Application::with_platform` with the locked SDK's benchmark platform. It retains the real application's assets and platform text system, and stays in `GpuiMode::Production`. Its windows have no desktop surface.

1. A CPU frame includes raw input/state work and ready executor work before that draw, then render, layout, prepaint, scene paint and callback cleanup. Synchronous draws inside native key dispatch count. Later draws get separate samples. Work after the final draw is charged to the final recorded frame. These are wall durations around CPU work, including waits and descheduling. The SDK dispatcher drain waits for queued or running background work as well, and its wait is charged to the recorded frame. Target lookup, scene fingerprints, journal collection and harness bookkeeping stay outside CPU work timers.
2. View samples request a full refresh and deliver a requested production frame callback. They perform actual rendering rather than measuring clean idle windows. Interaction samples dispatch raw platform events against the preceding settled frame. Ready fixture work and production follow-up draws count. Every measured input must change its scene or input probe. Missing draws, lost journal entries, throttled frames and incomplete sample sets fail the harness.
3. The CPU gate checks each run's p95, each rendered frame ordinal and the worst frame per input. It also includes presentation-only callbacks. A cheap frame cannot hide a later expensive frame. The required p95 ceiling is exactly `8.33` milliseconds, which rounds `1000 / 120` down.
4. Input-to-content-settled time is the unpaced wall interval through the last content draw and its callback cleanup. It includes bookkeeping between frames. Draw counts and ordinal timings make multi-frame interactions visible. A following renderless completion callback establishes content quiescence. Production VRR keepalive presentation is sampled and gated separately. It does not extend content-settled latency because it presents the same scene.
5. The harness does not wait for future timers, such as continued caret blinking. Fixture reads resolve immediately, so network, cluster and external IO latency are absent. Setup, revealing a control, pagination to the target and warm-up are excluded. The reported first warm-up draw follows setup and is not cold-open time. Results establish the recorded warmed workload, not first visits, every timer state or all possible data distributions.
6. The primary CPU pass has no GPU renderer. It covers CPU work through scene construction. A separate warmed renderer-enabled replay records per-frame CPU work including encoding, texture uploads and queue submission. Those frames also stay in the CPU gate. The explicit offscreen readback is another measurement, including encode, submission, GPU completion wait and RGBA copy. It does not isolate GPU execution or display present. The API exposes neither an adapter identity nor display-present timestamps.

The window is 1440 by 900 logical pixels. The SDK's scale factor is 2, so offscreen PNGs contain 2880 by 1800 physical pixels. Renderer-enabled frame callbacks also submit scenes before the explicit readback. Each sample's readback waits for queued work, draining earlier submissions before the next input. This replay does not reproduce display pacing or sustained GPU queue pressure. Do not add CPU and readback percentiles, or subtract the empty-scene floor, to infer displayed frame time.

Percentiles use nearest rank within each run. Reported case p50, p95 and p99 are medians of the per-run statistics. An even run count averages the two central values. Max is the largest sample in any run. The p95 spread is the minimum and maximum per-run p95. The empty-scene readback floor pools the equally sized runs. Raw files retain every frame and the SDK's request-layout, prepaint and paint phases. Half-run medians show warm-up drift. Load averages are recorded before and after every run. Passing the median alone does not pass the gate.

## Test-mode redraw evidence

The locked backend is `gpui-pre 0.3.11`, selected by `gpui-kit 0.9.0`. Inspect that crate's source under Cargo's registry directory:

1. `src/app/headless_app_context.rs:92` sets the inherited headless test context to `GpuiMode::test()`.
2. `src/app.rs:2182`, `flush_effects`, draws dirty windows immediately only inside the `GpuiMode::Test` branch. Production schedules the platform frame source instead.
3. `src/app.rs:242`, `Application::with_platform`, calls `App::new_app`. The constructor sets `GpuiMode::Production` at line 1027. This harness never changes it.
4. `src/window.rs:2007` runs the production frame callback, including next-frame callbacks, dirty-state checks, drawing, presenting and arena cleanup. The wrapper delivers that registered callback through `simulate_scheduled_frame`. It never substitutes `render_frame` during a measured input.
5. `src/window.rs:6268` can draw before dispatching a key even in Production. The timestamp journal retains this draw and carries subsequent state work into the next frame. Only the immediate test-mode effect-flush draws disappear.
6. `src/window.rs:1390` gives high-rate input a one-second presentation sustain period. Waiting for all frame demand to stop creates an artificial unpaced busy loop. This harness records the renderless completion callback separately and settles content after all ready work and content draws finish. It does not label production keepalive as test-only.
7. `src/platform.rs:1471` defines `PlatformHeadlessRenderer::render_scene` as the CPU encode/submit path without completion wait or readback. `src/platform/test/window.rs:558` invokes it. `Window::present` at `src/window.rs:3836` wraps this call, so callback CPU includes it. Its `render_to_image` path includes readback. Physical display present and isolated GPU execution remain unavailable.
8. `src/profiler/journal.rs:703` records a content submit as `IntervalBoundary::Presented` and a renderless submit as `ForegroundEvent::Present`. The harness reads both. The submit interval includes the headless encoder and queue submission, not physical display presentation.

The dev-only `gpui-frame-engine` alias enables the locked backend's `bench-support` API. It does not select another engine. SDK trace instrumentation remains enabled in optimized measurements, and its cost remains in CPU work timers. Version 1 reports used Test mode and summed several draws into an interaction latency. They cannot serve as the per-frame comparator. The runner rejects comparisons across measurement versions.

## Fixtures and coverage

`tests/frames/fixture.rs` replicates the browser-mock export to benchmark cardinalities. It asserts each reached count and records its source in raw reports:

| Input | Count | Source |
| --- | ---: | --- |
| Resource rows | 10,000 | `benchmarks/resource-table.bench.ts`, `rows` across 100 synthetic namespaces |
| Map nodes | 4,000 | `benchmarks/topology-large.bench.ts`, `buildOwnershipTopology(2_000)` |
| Resource events | 1,000 | `benchmarks/resource-detail.bench.ts`, `events.length`, including 500 warnings |
| Log lines | 1,000 | `benchmarks/resource-detail.bench.ts`, `logLines`; `src/features/resource-detail/log-helpers.ts`, `MAX_RETAINED_LOG_LINES` |
| Incidents | 10,000 | `benchmarks/incidents.bench.ts`, `incidentItems.length` |
| Port forwards | 500 | `benchmarks/live-sessions.bench.ts`, `portForwardSessions` |
| Exec sessions | 250 | `benchmarks/live-sessions.bench.ts`, `podExecSessions` |

These are replicated product fixtures at benchmark sizes, not the benchmarks' full data distributions. Scope filtering, grouping, selection and viewport culling run normally. The selected Deployment's 1,000 events produce 500 timeline entries.

`tests/frames/cases.rs` and its modules register all 57 parity views and 51 interaction loops. The original 27 cases remain. `extra.rs` covers detail close, timeline expansion/collapse, event search, log controls and exec scrolling. `interactions.rs` covers watch/output bursts, terminal selection, typing, mode switches, map movement and both detail scroll paths. `navigation.rs` covers workspace editing, forward start and stop, Helm details, settings categories, Argo documents/review, Flux mode and resource selection. `controls.rs` covers diagnostics, mock MCP controls and every RBAC category. Registered coverage must still pass setup, effective-input checks and the per-run gate before it counts as measured evidence. Resource pages hold 50 rows, as `model::PAGE_SIZE` defines. Grouping, list virtualization and map culling run normally. This inventory does not establish every menu, guarded-operation state, keyboard path, viewport, platform or page being ported on other branches. Shared parity scenarios establish behavior separately. Displayed 120 fps requires GPU and present evidence beyond this headless CPU gate.

The original events and timeline scroll probes exposed a real overflow bug. Their immediate flex body hid the tall content from the scroll handle. Shared scenarios reproduce the failure and check actual anchor movement in both drivers. The native sizing fix makes those bodies non-flexing. Both frame probes now require every measured wheel input to move the scene; idle inputs still fail.

Watch bursts use `export-watch-burst.ts`'s 10,000 records. Each repetition prepares owned kind responses and notifications before timing, then delivers all 10,000 notifications through the live channels inside the timer. The native backend constructs these payloads and refills returned row buffers on its runtime rather than on the UI thread. The fixture releases returned buffers during its next producer-preparation phase. Timed work includes channel delivery, watch collection, reconciliation, filtering, sorting and drawing. Health alternates so every repetition must change the scene while retaining all rows. Between repetitions, setup waits the existing `RESOURCE_WATCH_COALESCE` interval and settles its cleanup. This measures a burst after that throttle, not sustained arrivals during the throttle or network latency.

Watch reconciliation validates the complete ordered read and notification coverage before exchanging row owners. When the read covers the whole source, it exchanges the vector owners instead of swapping every row record. Partial kinds keep indexed replacement, and incomplete notice sets keep named reconciliation. Prepared key identity survives releasing the visible row owner. Prepared kind/API variants evaluate the current kind selection once per variant. Filtering still evaluates current workspace and resource inclusion predicates. Display order is retained only when membership, prepared keys, preferred-owner flags and kind-lane order match; health totals always read fresh rows. Map view reuse compares selection, expanded groups and mode explicitly, and replacing the topology discards its derived data.

The existing edge snapshot also retains exact tessellated paths for the current origin, viewport and visible edge indices. Geometry changes rebuild those paths using the original absolute-coordinate construction. Colors and the current content mask are applied at paint time; paths for invisible edges are not tessellated.

Terminal output bursts replay the retained 1,000-line log fixture into the mock exec stream. The last line includes the repetition index so identical output cannot count as a change. The fixture backend handles all MCP, Argo, forward and exec actions in memory. No real server, cluster, credentials or user settings participate.

## Investigation record

On Linux, raw samples also retain UI-thread scheduler CPU and runqueue-wait counters before and after each whole sample. They include bookkeeping and use kernel-updated counters, so they are diagnostics rather than precise per-frame CPU clocks. They never replace or subtract from the wall-time gate. Other platforms report them as unavailable. See the [Linux scheduler statistics contract](https://www.kernel.org/doc/html/latest/scheduler/sched-stats.html).

Measurements compiled in shared Cargo directories are untrusted because concurrent worktrees could overwrite path-crate artifacts. They cannot establish a baseline or a speedup. Both sides of the final comparison must be rebuilt in this worktree's isolated directories with the corrected version 2 harness. Earlier Test-mode reports also remain historical diagnostics.

The imported handback contains historical `perf record` evidence. The opt-in `frame-profile` feature records named element and data-processing phases during measured work. Setup, resets and warm-up are excluded from those spans. Each span includes its measured input number, so notification and row-work totals can be compared with that input's scheduler counters. Those instrumented builds are diagnostic only; the frame gate rejects them. The SDK journal records layout, prepaint and paint timings within every measured draw. The port report records profiles, distributions, settling counts, load averages and verification results.

The current sidebar retains HEAD's expandable namespace tree, keyboard focus and reveal behavior. The earlier flat namespace list's virtualization was not copied over that newer tree. Resource table rows use measured fixed geometry to construct only intersecting rows. The imported handback's historical pixel comparisons do not verify this port; its checks and measurements belong in `frames-report.md`.

Content-settled latency ends at the last content draw's callback completion. Remaining ready work that produces no draw is charged to the last CPU sample. That conservative tail can make frame CPU slightly exceed content-settled latency. The readback replay re-encodes the retained scene after the production submit and can wait for earlier queued work; it is not an additive display latency or an isolated GPU execution time.
