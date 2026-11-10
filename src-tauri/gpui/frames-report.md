# GPUI frames port

The port, required checks and focused parity pass. The 120 fps gate remains red. After five measured optimizations, 15 of 22 cases in final three-run validation pass and seven exceed 8.33 ms. The full 108-case baseline and final focused results are recorded below. Each remaining case has a measured cause. Work stops at the requested measured-cause condition; the 120 fps goal is not achieved.

## Source and scope

Base: `a2239cc`. Parent parity fixes were cherry-picked as `d427251` and `dbe7b79`. Port commit: `d0860ac`. Harness alignment commit: `5806142`. Both commits' pre-commit hooks passed, including `cargo check --workspace --manifest-path src-tauri/Cargo.toml`. The older 82-file patch was ported by intent. The missing earlier report was waived by the user. The earlier worktrees were read without modification. Their timings are historical evidence only.

The port adds the 108-case headless production-mode harness, prepared resource rows and watch patches, indexed filtering, a kind-selection mask, map edge culling and topology preparation comparisons. It fixes Events and Details timeline overflow with shared scroll scenarios. It preserves the current expandable namespace tree, resource controls, custom-resource state, map focus controls, retry behavior and concurrent parity runner.

| Commit | Outcome |
| --- | --- |
| `d427251` | Parent retry waits and friendly dry-run error assertion |
| `dbe7b79` | Parent concurrent parity drivers, shards and page pool |
| `d0860ac` | Native frame harness and prepared rendering port |
| `5806142` | Current workspace startup and all-namespace fixture alignment |
| `9b0312f` | Reject frame runs overlapping native parity |
| `ae76428` | Row/component profiling and parent parity-process guard |
| `7083b6e` | Reuse row projections when no owner resolves |
| `07c03de` | Reuse prepared summary metadata |
| `64d2d50` | Reuse owner keys in indexed filtering |
| `4d69cf0` | Borrow ownership lookup keys |
| `24d19ac` | Additional UI construction and fixture profiling |
| `5be5d9a` | Reuse focus registration for unchanged map views |

## Verification

| Check | Result |
| --- | --- |
| `cargo fmt` and formatting check | Pass |
| `cargo clippy --all-targets -- -D warnings` | Pass at `5be5d9a` |
| `cargo clippy --all-targets --features frame-profile -- -D warnings` | Pass at `5be5d9a` |
| `cargo test --lib --test native` | 30 library and 85 native tests pass; one standalone watch benchmark ignored |
| `bun run typecheck` | Pass |
| `bun run lint` | Pass, with 14 existing warnings and two informational findings |
| `bun test e2e/parity/contract.test.ts` | Five tests pass, 20 assertions |
| `git diff --check` | Pass |
| Focused parity `events.,detail.,resources.,topology.` | All 21 scenarios pass in both drivers |

Latest expanded parity at `5be5d9a`: all 41 scenarios pass in both drivers in `e2e/artifacts/parity-e4b9d0d5-0cf1-4365-b593-02d1e318f6e8`. It covers `events.,detail.,resources.,topology.,resource-filters.,kind-picker.,network-flow.,metrics.,coverage-map.,custom-resources.discovery`, including graph keyboard skip, zoom and pan. No failed scenario required `PARITY_RERUN=last`.

After harness alignment, the same 30 library tests, 85 native tests and all 21 focused parity scenarios passed again. Latest parity artifact: `e2e/artifacts/parity-e386fd66-3723-47f5-8eaa-fe7c71d66db0`. No rerun was needed.

The current scenario catalog has no `topology.` IDs. Earlier focused runs covered `network-flow.`, `metrics.` and `coverage-map.` in both drivers. Additional passing groups were `resource-filters.` and `kind-picker.`. Those earlier runs are separate from the latest runner verification.

| Earlier group | Passing scenarios per driver | Artifact under `e2e/artifacts/` |
| --- | ---: | --- |
| `network-flow.` | 2 | `parity-b904ea75-dde7-4319-a1db-89902e2c4f1a` |
| `metrics.` | 2 | `parity-00ec71c0-c8a8-46f5-9edc-5b915c7d504d` |
| `coverage-map.` | 7 | `parity-5bf8bf4e-a7e4-4e99-b93f-9d030d3460f0` |
| `resource-filters.` | 6 | `parity-d0ed0bcb-0bbb-4dce-a05a-b34b8afba5cb` |
| `kind-picker.` | 2 | `parity-74428075-8a79-4b03-a230-169db73fd3d1` |

The former refused-YAML dry-run test now passes after the parent fix. A prior native parity run failed `custom-resources.discovery`; its latest focused run passes in both drivers after the all-namespace fixture correction. The expanded focused run at `24d19ac` passes all 41 scenarios per driver in `e2e/artifacts/parity-d60a9321-5df9-421f-8396-120df81dc36b`. No desktop windows or Xvfb were used.

## Frame protocol

The required gate uses 108 cases, comprising 57 views and 51 interaction loops. Each of three runs has 120 warm-up inputs and 240 measured inputs. CPU and renderer-enabled replay are measured separately in an optimized build. Every run, frame ordinal, presentation-only callback and worst frame per input must meet p95 at most 8.33 ms. Median p95 alone does not pass.

Every heavy command uses the shared `heavy.lock`, priority 10, worktree-local Cargo target/build directories, two build jobs and an empty `RUSTC_WRAPPER`. Measurement runs must have no concurrent parity process. The runner captures `uptime` before each repetition and load averages before and after it. Each measurement artifact records build paths and its pinned executable SHA-256.

Physical display presentation is unavailable in this headless harness. Its gate covers CPU frame work including renderer encoding and submission. Offscreen readback remains a separate diagnostic.

The viewport is 1440 by 900 logical pixels; offscreen renderer images are 2880 by 1800 pixels. The scaled inputs follow the existing benchmarks: 10,000 resource rows over 100 namespaces, 4,000 map nodes, 1,000 detail events, 1,000 retained log lines, 10,000 incident items, 500 port-forward sessions and 250 exec sessions. These sizes come from the harness's recorded benchmark sources, not new product limits.

The diagnostic renderer PNGs `events-scroll.png` and `timeline-scroll.png` were visually inspected. They show the intended workspace and clipped, populated detail content. Actual movement is established by the shared overflow scenarios and the 240/240 effective-input checks, not by the final still images.

## Final optimized validation

Source: `5be5d9a`. Artifact: `e2e/artifacts/gpui-frames-2026-11-06T13-08-38-241Z`. Pinned executable: `frames-executable`. SHA-256: `2a05d078c5fb3ec80922bf9a52def999006e7258eedf65ce7c2af8359279606b`. The manifest, target and build directories are the worktree-local paths recorded below for the baseline. This build disables profiling and debug assertions and uses production measurement version 2. Peak release-build RSS was 2387.7 MiB.

The same protocol measured all 19 baseline failures plus three additional network cases. Fifteen of these 22 pass in every run. Seven remain over budget. All interaction replays have 240/240 effective measured inputs. The runner exits 1 because the gate fails.

| Repetition | Uptime before measurement, local CET | Load before | Load after | Observed child priority |
| --- | --- | --- | --- | ---: |
| 1 | 14:10:20 up  4:06,  1 user,  load average: 4.08, 3.83, 2.90 | 4.09/3.83/2.9 | 2.2/2.25/2.5 | 16 |
| 2 | 14:18:45 up  4:15,  1 user,  load average: 2.20, 2.24, 2.49 | 2.2/2.25/2.5 | 2.46/2.06/2.25 | 16 |
| 3 | 14:27:11 up  4:23,  1 user,  load average: 2.46, 2.05, 2.25 | 2.46/2.06/2.25 | 2.34/2.09/2.18 | 16 |

CPU and renderer p95 below are medians across three repetitions. Worst p95 is the maximum of every gated series in every repetition, using unrounded values. Times are milliseconds.

| Case | CPU p95 | Renderer CPU p95 | Worst p95 | Gate |
| --- | ---: | ---: | ---: | --- |
| resources | 5.936 | 7.111 | 7.393 | Pass |
| table-scroll | 6.283 | 7.398 | 7.793 | Pass |
| detail-open | 8.954 | 9.630 | 9.813 | Over |
| detail-tabs | 7.400 | 8.415 | 8.423 | Over |
| search-typing | 10.613 | 11.773 | 12.048 | Over |
| event-filters | 5.452 | 6.820 | 6.825 | Pass |
| map-pan | 5.847 | 7.224 | 7.445 | Pass |
| map-zoom | 6.037 | 7.313 | 7.346 | Pass |
| detail-close | 9.106 | 9.646 | 10.039 | Over |
| live-limited | 5.551 | 6.697 | 6.867 | Pass |
| live-failed | 5.676 | 6.764 | 6.816 | Pass |
| live-reconnecting | 5.599 | 6.821 | 7.196 | Pass |
| network-flow | 6.855 | 7.800 | 7.998 | Pass |
| kind-picker | 6.085 | 7.062 | 7.190 | Pass |
| live-watch-burst | 8.360 | 8.704 | 9.090 | Over |
| exec-selection-drag | 4.281 | 5.528 | 5.565 | Pass |
| kind-picker-toggle | 12.487 | 13.948 | 15.270 | Over |
| kind-picker-typing | 6.419 | 7.568 | 8.064 | Pass |
| network-flow-pan | 6.993 | 8.276 | 8.301 | Pass |
| network-flow-zoom | 4.773 | 5.892 | 6.138 | Pass |
| gitops-application-open | 8.533 | 8.490 | 8.598 | Over |
| network-flow-selection | 6.528 | 6.328 | 6.908 | Pass |

Twelve of the 19 original failures now pass. The additional network cases also pass. The other 86 baseline passes were not remeasured after optimization, so this is focused final validation, not a new 108-case passing claim. The earlier watch-only pass did not repeat: its final renderer median is 8.704 ms and worst p95 is 9.090 ms.

## Remaining cases and measured causes

All seven failures have a measured operation or component responsible for material frame work. This satisfies the requested stopping condition; the 8.33 ms target remains unmet. The following component profile is diagnostic, not gate evidence.

Artifact: `e2e/artifacts/gpui-frames-profile-final-5be5d9a`. Source: `5be5d9a` plus the artifact's `instrumentation.patch`. Pinned binary SHA-256: `062cd76c06fe1b54d3466c3a5835dad1ace767e5aa1c8231485be110e60d5342`. Same worktree target/build paths, optimized build with `frame-profile`. Each case has 120 warm-up and 240 effective measured inputs in both replays. Uptime before: `14:38:06 up 4:34, 1 user, load average: 2.64, 2.21, 2.20`; after: `14:44:19 up 4:40, 1 user, load average: 2.64, 2.24, 2.19`.

Component values below are p95 of each span's total within one input. CPU and renderer values are separate. Nested spans and independent percentiles must not be added. Multiple draws are reported because a per-input component total is not one frame's duration.

| Case | Final worst p95, ms | Measured cause on final code |
| --- | ---: | --- |
| detail-open | 9.813 | Changed map views repeat focus reconciliation at 1.108 / 1.171 ms and edge preparation at 1.437 / 1.445 ms per input, over two draws. Inspector construction is 2.840 / 2.955 ms per input. Detail construction itself is only 0.044 / 0.044 ms. |
| detail-tabs | 8.423 | YAML body request/layout contributes 2.487 / 2.521 ms at overall input p95. Within the 40 YAML inputs per replay, its conditional p95 is 2.555 / 2.576 ms. This accounts for nearly all the detail pane's request/layout cost. The exact editor subroutine is not isolated. |
| search-typing | 12.048 | Input update costs 2.115 / 2.376 ms, including filtering and map preparation. Changed-view focus reconciliation adds 1.526 / 1.638 ms inside Inspector construction. Map preparation is 1.252 / 1.291 ms and row filtering is 0.810 / 1.027 ms per input. The unchanged-view cache cannot reuse these changed views. |
| detail-close | 10.039 | Map focus reconciliation costs 2.378 / 2.348 ms across 2.004 draws per input. Inspector construction costs 3.982 / 3.978 ms and page prepaint 2.642 / 2.634 ms per input. Closing redraws the resource map and table; input dispatch itself is 0.222 / 0.221 ms. |
| live-watch-burst | 9.090 | Watch flush remains inside render at 2.828 / 2.694 ms. Six ordered patch validations per input consume 1.450 / 1.382 ms within patch work of 1.674 / 1.586 ms. Filtering adds 1.167 / 1.080 ms. This is repeated snapshot validation and filtering, not the earlier full row projection. |
| kind-picker-toggle | 15.270 | Input dispatch costs 4.264 / 4.295 ms, followed by about 3.013 draws per input. Filtering is 1.685 / 1.614 ms, map preparation 1.625 / 1.479 ms and changed-view focus reconciliation 1.981 / 1.937 ms. Fixture kind reads cost 1.015 / 1.153 ms at overall input p95 and occur on half the inputs. Live reconciliation is 0.388 / 0.422 ms. |
| gitops-application-open | 8.598 | The SDK journal identifies the Argo completion task at `src/inspector/argo_tab.rs:98`: 2.547 ms p95 across 480 retained polls, one per measured input. It creates `ArgoView`, all change highlights and editor pairs before drawing. The task's internal costs have not been split further. |

The actual nearest-rank p95 witnesses below reproduce every final worst p95 exactly. Request/layout, prepaint and paint are timings from that same frame. "Outside draw" is CPU frame time minus SDK draw time. It includes assigned update work and callback work after drawing, including submission; it must not be labeled solely as work before drawing. Runqueue covers the whole input and cannot be assigned to an individual phase when the input draws more than once.

| Case | Run, replay, series | Samples | P95 frame | Request/layout | Prepaint | Paint | Outside draw | Whole-input runqueue |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| detail-open | 1, renderer, worst-input | 240 | 9.813 | 3.034 | 3.360 | 1.831 | 1.173 | 0.000 |
| detail-tabs | 3, renderer, content | 241 | 8.423 | 3.570 | 2.424 | 1.094 | 0.975 | 0.000 |
| search-typing | 3, renderer, content | 240 | 12.048 | 3.473 | 3.515 | 1.703 | 2.980 | 0.000 |
| detail-close | 3, renderer, worst-input | 240 | 10.039 | 3.904 | 3.389 | 1.398 | 0.965 | 0.000 |
| live-watch-burst | 1, renderer, content | 240 | 9.090 | 4.024 | 2.444 | 0.733 | 1.652 | 0.000 |
| kind-picker-toggle | 2, renderer, worst-input | 240 | 15.270 | 4.173 | 3.468 | 1.658 | 5.541 | 0.047 |
| gitops-application-open | 2, cpu, content | 240 | 8.598 | 0.898 | 3.306 | 1.171 | 2.920 | 0.000 |

### GitOps task isolation

Three narrower profiles separate the additional work from rendering. Each uses the same 120 warm-up and 240 effective inputs in CPU and renderer replay, both locks and the process guard. All executable paths below are `e2e/artifacts/<artifact>/frames-executable`, with the same worktree Cargo target/build directories. Their temporary source changes are retained as `instrumentation.patch` in each artifact and were removed from the working source afterward.

| Artifact | SHA-256 | Uptime before, local CET | Uptime after, local CET |
| --- | --- | --- | --- |
| gpui-frames-profile-gitops-settle | `37799018ab27569c11d00b66f296337cdad1a7bbd6ff9a23209e28f3222555b9` | 14:55:33 up 4:51, load 4.18/2.33/2.06 | 14:56:09 up 4:52, load 3.33/2.31/2.06 |
| gpui-frames-profile-gitops-events | `6267e0a4f34a117ed3fbea3f38e7f0202a50714c239d4277d34df19c2391be2d` | 15:03:13 up 4:59, load 3.06/1.95/1.88 | 15:03:48 up 5:00, load 2.55/1.94/1.88 |
| gpui-frames-profile-gitops-tasks | `8bbde34ae9beae006a5391f8872f73f925791216abdeb14dc36dd9c7662948c6` | 15:07:41 up 5:04, load 1.37/1.46/1.68 | 15:08:17 up 5:04, load 1.58/1.51/1.69 |

The settle profile measures queued foreground work at 2.620 / 2.645 ms p95, before the SDK draw. Detail settling is 0.080 / 0.104 ms, requested-tab setup 0.025 / 0.052 ms and memory preference saves 0.014 / 0.014 ms. The task profile records 2.547 ms p95 specifically at the Argo completion task's spawn location. The SDK journal's existing 100 microsecond task-poll floor is defined by `gpui-pre 0.3.11/src/profiler/journal.rs:39`; all 480 expected Argo polls were retained.

A task-profile p95 renderer witness records 2.724 ms before drawing, 5.439 ms inside drawing and 0.234 ms after drawing, summing to 8.396 ms. This is diagnostic timing, not the final gate value. Event preparation is 0.0008 ms and event sorting 0.00005 ms for this Application case, so event sorting was rejected as its bottleneck. The Argo snapshot preparation is the measured foreground cost.

## Three-run baseline

Artifact: `e2e/artifacts/gpui-frames-2026-11-06T10-34-20-723Z`. All 57 views and 51 loops completed three runs, with 120 warm-up and 240 measured inputs in each CPU and renderer replay. Every loop had 240 effective inputs in both replays. Both existing locks were held, and the isolation guard detected no parity overlap.

The measured Rust source is `5806142`. The runner includes `9b0312f` and its subsequent runner-process guard extension. The Rust binary is pinned in the artifact as `frames-executable`.

SHA-256: `ebc8eeb3c0c9f48c16f1ff798db04e345b541487aa39c0444a0826a131d8f5cf`.

Manifest directory: `src-tauri/gpui`. Target: `src-tauri/gpui/target`. Build: `src-tauri/gpui/build`. Optimized build, profiling disabled, debug assertions disabled. CPU: AMD Ryzen 7 9800X3D 8-Core Processor, 16 logical processors.

| Repetition | Uptime before measurement, local CET | Load before | Load after | Observed child priority |
| --- | --- | --- | --- | ---: |
| 1 | 11:34:20 up  1:30,  1 user,  load average: 9.93, 16.33, 14.96 | 9.93/16.34/14.96 | 1.85/2.11/5.43 | 6 |
| 2 | 11:54:14 up  1:50,  1 user,  load average: 1.85, 2.11, 5.43 | 1.85/2.11/5.43 | 2.61/2.29/3.05 | 16 |
| 3 | 12:14:28 up  2:10,  1 user,  load average: 2.61, 2.28, 3.05 | 2.61/2.29/3.05 | 1.27/1.65/2.09 | 16 |

The command applies `nice -n 10` once. The table records the child's observed priority rather than assuming a fixed inherited value. The starting load retains preceding parity history; the locks and process guard establish isolation during this run.

CPU and renderer p95 are medians across the three runs. Worst p95 is the highest p95 in any gated series and any run, including frame ordinals, worst frames per input and presentation-only callbacks. All times are milliseconds. The gate uses unrounded values and a ceiling of exactly 8.33 ms.

| Case | Kind | CPU p95 | Renderer CPU p95 | Worst p95 | Gate |
| --- | --- | ---: | ---: | ---: | --- |
| overview | view | 3.063 | 3.491 | 3.721 | Pass |
| resources | view | 7.090 | 8.089 | 9.127 | Over |
| detail | view | 5.266 | 6.581 | 6.620 | Pass |
| yaml | view | 4.341 | 5.270 | 5.843 | Pass |
| events | view | 4.882 | 5.765 | 6.244 | Pass |
| logs | view | 5.005 | 5.337 | 6.961 | Pass |
| revisions | view | 4.579 | 4.901 | 6.061 | Pass |
| actions | view | 4.701 | 5.348 | 6.325 | Pass |
| cluster-overview | view | 4.294 | 4.896 | 5.640 | Pass |
| incidents | view | 3.223 | 3.523 | 3.769 | Pass |
| port-forwards | view | 4.277 | 4.516 | 4.881 | Pass |
| table-scroll | interaction | 7.241 | 8.275 | 8.678 | Over |
| log-scroll | interaction | 4.780 | 6.005 | 6.650 | Pass |
| detail-open | interaction | 8.390 | 9.482 | 13.753 | Over |
| detail-tabs | interaction | 7.777 | 8.312 | 8.333 | Over |
| search-typing | interaction | 12.894 | 14.894 | 15.313 | Over |
| log-filter-typing | interaction | 5.164 | 6.020 | 6.431 | Pass |
| event-filters | interaction | 5.441 | 6.980 | 12.629 | Over |
| incident-filters | interaction | 4.287 | 4.496 | 4.632 | Pass |
| map-pan | interaction | 6.753 | 8.236 | 8.543 | Over |
| map-zoom | interaction | 7.097 | 8.550 | 14.366 | Over |
| exec-session-scroll | interaction | 5.208 | 5.838 | 6.297 | Pass |
| detail-close | interaction | 8.702 | 9.017 | 10.237 | Over |
| timeline-toggle-all | interaction | 5.584 | 6.850 | 7.260 | Pass |
| events-search-typing | interaction | 5.218 | 6.579 | 6.907 | Pass |
| log-wrap | interaction | 5.597 | 6.523 | 6.908 | Pass |
| log-order | interaction | 5.400 | 6.470 | 6.704 | Pass |
| forward-start | view | 3.886 | 5.012 | 5.324 | Pass |
| forward-active | view | 4.126 | 5.267 | 5.302 | Pass |
| exec-session | view | 3.863 | 5.076 | 5.313 | Pass |
| live-limited | view | 6.804 | 8.381 | 8.547 | Over |
| live-failed | view | 6.910 | 8.525 | 8.887 | Over |
| live-reconnecting | view | 6.669 | 7.982 | 8.930 | Over |
| custom-resources | view | 4.254 | 4.872 | 5.057 | Pass |
| network-flow | view | 6.691 | 7.436 | 7.944 | Pass |
| metrics | view | 5.796 | 6.748 | 7.036 | Pass |
| usage-footer | view | 2.885 | 3.471 | 3.813 | Pass |
| actions-image | view | 4.375 | 5.544 | 5.583 | Pass |
| actions-environment | view | 4.674 | 5.976 | 6.228 | Pass |
| actions-resources | view | 4.766 | 6.348 | 6.382 | Pass |
| actions-labels | view | 4.531 | 5.920 | 6.468 | Pass |
| actions-annotations | view | 4.331 | 5.609 | 5.858 | Pass |
| actions-rollback | view | 4.488 | 5.379 | 5.664 | Pass |
| yaml-edit | view | 4.504 | 5.894 | 6.227 | Pass |
| yaml-kyaml | view | 4.413 | 5.834 | 6.127 | Pass |
| yaml-edit-kyaml | view | 4.417 | 4.885 | 5.118 | Pass |
| rbac | view | 4.002 | 4.578 | 4.707 | Pass |
| workspaces | view | 1.384 | 1.778 | 1.799 | Pass |
| settings | view | 3.028 | 3.599 | 3.832 | Pass |
| palette | view | 3.963 | 4.875 | 5.243 | Pass |
| gitops | view | 3.386 | 4.395 | 4.651 | Pass |
| gitops-list | view | 3.362 | 3.760 | 4.416 | Pass |
| flux | view | 3.033 | 3.546 | 3.781 | Pass |
| argo-app | view | 4.282 | 4.791 | 5.376 | Pass |
| argo-workspace | view | 4.416 | 5.492 | 6.275 | Pass |
| helm | view | 3.029 | 3.664 | 3.748 | Pass |
| helm-list | view | 3.057 | 3.325 | 3.662 | Pass |
| helm-release | view | 3.276 | 3.658 | 3.958 | Pass |
| settings-agents | view | 2.649 | 3.153 | 3.891 | Pass |
| settings-appearance | view | 3.171 | 3.348 | 3.921 | Pass |
| settings-sessions | view | 2.639 | 2.807 | 3.208 | Pass |
| settings-yaml | view | 2.681 | 3.067 | 3.500 | Pass |
| settings-diagnostics | view | 2.934 | 3.508 | 3.587 | Pass |
| settings-advanced | view | 2.468 | 3.042 | 3.202 | Pass |
| settings-remote | view | 2.584 | 2.867 | 3.585 | Pass |
| settings-updates | view | 2.462 | 2.775 | 3.176 | Pass |
| settings-kubeconfig | view | 2.766 | 3.111 | 3.405 | Pass |
| argo-connected | view | 4.616 | 5.047 | 5.575 | Pass |
| argo-review | view | 4.395 | 5.167 | 5.633 | Pass |
| argo-desired | view | 4.892 | 5.354 | 6.242 | Pass |
| workspace-editor | view | 1.372 | 1.632 | 1.747 | Pass |
| custom-search | view | 3.468 | 3.829 | 3.927 | Pass |
| kind-picker | view | 6.922 | 8.682 | 8.844 | Over |
| live-watch-burst | interaction | 28.347 | 34.125 | 43.174 | Over |
| exec-output-burst | interaction | 5.515 | 7.152 | 7.593 | Pass |
| exec-selection-drag | interaction | 4.048 | 4.726 | 9.219 | Over |
| helm-search-typing | interaction | 3.099 | 3.507 | 3.977 | Pass |
| kind-picker-toggle | interaction | 13.841 | 15.134 | 19.578 | Over |
| gitops-view-toggle | interaction | 3.951 | 4.553 | 4.937 | Pass |
| helm-view-toggle | interaction | 3.158 | 3.656 | 3.858 | Pass |
| palette-typing | interaction | 4.828 | 5.211 | 5.270 | Pass |
| kind-picker-typing | interaction | 7.190 | 8.913 | 9.183 | Over |
| custom-search-typing | interaction | 3.314 | 3.706 | 3.812 | Pass |
| settings-search-typing | interaction | 6.162 | 6.468 | 6.507 | Pass |
| network-flow-pan | interaction | 6.603 | 7.653 | 8.465 | Over |
| network-flow-zoom | interaction | 4.577 | 5.395 | 5.725 | Pass |
| metrics-pan | interaction | 5.691 | 7.149 | 7.649 | Pass |
| metrics-zoom | interaction | 4.225 | 5.289 | 5.394 | Pass |
| events-scroll | interaction | 5.224 | 6.683 | 6.747 | Pass |
| timeline-scroll | interaction | 5.908 | 7.120 | 7.293 | Pass |
| workspace-editor-open | interaction | 1.486 | 1.614 | 1.636 | Pass |
| forward-start-typing | interaction | 3.983 | 4.916 | 4.981 | Pass |
| forward-start-submit | interaction | 4.538 | 4.970 | 5.133 | Pass |
| forward-stop | interaction | 4.483 | 4.799 | 4.918 | Pass |
| helm-release-open | interaction | 4.879 | 4.991 | 5.093 | Pass |
| workspace-editor-typing | interaction | 1.551 | 1.725 | 1.757 | Pass |
| rbac-typing | interaction | 4.833 | 5.094 | 5.184 | Pass |
| settings-page-navigation | interaction | 3.345 | 3.629 | 3.764 | Pass |
| argo-document-navigation | interaction | 4.695 | 4.904 | 4.954 | Pass |
| argo-review-open | interaction | 4.811 | 4.802 | 5.147 | Pass |
| flux-view-toggle | interaction | 3.071 | 3.584 | 3.787 | Pass |
| gitops-application-open | interaction | 8.025 | 8.567 | 8.704 | Over |
| network-flow-selection | interaction | 5.666 | 5.936 | 6.240 | Pass |
| metrics-selection | interaction | 6.541 | 7.006 | 7.596 | Pass |
| settings-diagnostics-toggle | interaction | 4.629 | 5.612 | 5.637 | Pass |
| settings-agents-toggle | interaction | 2.858 | 3.063 | 3.234 | Pass |
| settings-agents-port-typing | interaction | 2.777 | 3.133 | 3.339 | Pass |
| rbac-category-navigation | interaction | 4.539 | 4.964 | 4.992 | Pass |

The first full repetition's table was reported while repetition 2 continued and remains in [frames-first-run.md](frames-first-run.md). It had 99 passing cases and nine over budget. The completed spread finds 19 over budget, so the first repetition alone was insufficient.

## Investigation record

Earlier attempts are excluded from acceptance. `gpui-frames-2026-11-06T09-32-51-828Z` measured the startup launcher before failing; the saved-workspace seam fixed setup. `gpui-frames-2026-11-06T09-38-52-244Z` stopped at the 8,334/10,000 watch-notification assertion; the all-namespace fixture filter omitted Widgets. `gpui-frames-2026-11-06T09-49-48-247Z` stopped at the missing summary accessibility ID. Commit `5806142` fixes these harness seams without weakening the effective-input or row-count assertions.

The single-run remaining-loop diagnostic is `gpui-frames-2026-11-06T09-51-42-476Z/report.md`. All 35 loops completed; 12 exceeded the ceiling. It is not three-run acceptance evidence.

The full attempt `gpui-frames-2026-11-06T10-09-57-372Z` was cancelled after external native parity shards and Chrome pages appeared while the required GPUI `heavy.lock` was held. The other runner used `/tmp/kubecove-parity.lock`. Only this task's frame process was terminated, and its exit was observed. A guard retry blocked overlap before measurement. Subsequent measurements acquire both existing locks. No other process or host setting was changed.

The first complete repetition measured watch request-layout p95 at 27.074 ms and renderer CPU p95 at 31.371 ms. This identifies the expensive phase, not yet the responsible operation. Additional diagnostic spans separate row source projection, inherited owners, summary filtering, sidebar and detail work. Instrumented builds cannot satisfy the acceptance gate.

Diagnostic instrumentation and the expanded parity guard were committed as `ae76428`. Default and `frame-profile` Clippy checks pass, as do its pre-commit hooks. The focused CPU profile uses `e2e/artifacts/gpui-frames-profile-ae76428`, source `ae76428`, binary SHA-256 `f8d8a3d611e1882ecdc0ba83ff23c227faf9fd7bacb16cca5fb2dc53366c9c30`. It uses 120 warm-up and 240 measured inputs for watch burst, search typing and kind selection. GPU replay is disabled for this diagnostic. Uptime before measurement: `12:39:10 up 2:35, 1 user, load average: 2.54, 2.26, 2.21`. Both locks and the isolation guard remain active.

The watch profile measures row-source projection at 11.194 ms p95 per input, including owner inheritance at 4.983 ms and key preparation at 6.133 ms. Watch flush is 20.980 ms p95; the source snapshot is also retired there. These percentiles are independently calculated and must not be added. The first optimization returns borrowed rows when no ownership parent resolves, then retains the existing prepared rows when no cluster rows must be joined. Actual inheritance and cluster joins keep the owned projection. Both callers were migrated. The watch assertion still requires 10,000 retained rows and 240 effective measured inputs.

The first uninstrumented remeasurement is `e2e/artifacts/gpui-frames-2026-11-06T11-43-17-546Z`. It is one run, with the same 120 warm-up and 240 measured inputs and both CPU and renderer passes. CPU p95 is 9.457 ms, renderer CPU p95 is 10.032 ms, and worst gated p95 is 10.032 ms. The baseline three-run median CPU p95 was 28.347 ms and renderer median p95 was 34.125 ms. This improvement does not satisfy 8.33 ms or replace the three-run acceptance protocol. Uptime: `12:44:59 up 2:41, 1 user, load average: 3.11, 2.40, 2.23`; ending load: `[0.98, 1.71, 1.99]`. Both locks and the overlap guard were active. Default Clippy passes, as do all 30 library and 85 native tests, with one existing manual benchmark ignored. The binary is pinned in that artifact, SHA-256 `6b827fd27c48a4e5ec5dd4740aa1b99bf4e226a56b81aebe2c123d2bfe1ab2a9`, with the same target and build paths as the baseline. The optimization was committed afterward as `7083b6e`; a documentation comment was corrected after this binary was built.

The next diagnostic profiles all 19 baseline failures from source `7083b6e`, with 120 warm-up and 240 measured inputs in CPU and renderer replay. Artifact: `e2e/artifacts/gpui-frames-profile-7083b6e`. Instrumented binary SHA-256: `9c9bd682bcfb9f1d05c83aca0ad9a847caf4af4784e41b0a01ce0a2ec66fcd05`. Uptime: `12:51:31 up 2:47, 1 user, load average: 2.23, 1.98, 2.02`. Both locks and the process guard are active. This diagnostic cannot pass the acceptance gate.

The first CPU profile measured search summary filtering at 2.330 ms p95 and full filtering at 3.692 ms. Kind toggling had 2.796 ms filtering, 1.583 ms summary work and 1.055 ms fixture kind-read work per input at p95. These were measured candidates, not acceptance results.

The all-failure profile confirms 2.696 ms summary work for search and 1.965 ms for watch burst in CPU replay. Commit `07c03de` makes summary filtering reuse prepared search strings and owner labels, copying only distinct menu entries. Cached fields are rebuilt by the existing watch key invalidation. The uninstrumented remeasurement is `e2e/artifacts/gpui-frames-2026-11-06T12-06-37-434Z`. Its pinned binary SHA-256 is `207d4589b9c1965e28dc4d9ed4931e371578e24d3aa586e58e6c47247902e0da`, built from the subsequently committed patch with unchanged target/build paths. One run, 120 warm-up and 240 measured inputs in both replays: search CPU p95 11.303 ms and renderer p95 12.591 ms; watch CPU p95 9.361 ms and renderer p95 9.719 ms. Both remain over budget. Uptime: `13:08:19 up 3:04, 1 user, load average: 3.88, 2.53, 2.00`; ending load `[1.50, 2.00, 1.90]`. Both locks and the overlap guard were active. Clippy and all 30 library and 85 native tests pass. Its pre-commit hook passes.

Commit `64d2d50` passes the cached owner key into the indexed refilter predicate, whose measured CPU p95 was 1.327 ms for watch and 1.417 ms for search at `7083b6e`. Every caller migrated to the new predicate signature. This avoids a second owner-label allocation pass while preserving scope, namespace, owner and health checks. Default Clippy and all 30 library and 85 native tests pass, with one existing manual benchmark ignored. Its pre-commit hooks pass.

Its uninstrumented, single-run remeasurement is `e2e/artifacts/gpui-frames-2026-11-06T12-15-09-338Z`, with 120 warm-up and 240 measured inputs in both replays. Search CPU p95 is 10.332 ms and renderer p95 is 11.613 ms. Watch CPU p95 is 8.297 ms and renderer p95 is 8.475 ms. Both still fail the full gate. Pinned binary SHA-256: `08c6b75080998645368c194ebf820d688057677670d7a2f3bee642e4054045fe`; source is the patch subsequently committed as `64d2d50`, with the same target/build directories. Uptime: `13:16:51 up 3:13, 1 user, load average: 3.62, 2.66, 2.14`; ending load `[1.30, 1.97, 1.97]`. Both locks and the guard were active.

Commit `4d69cf0` removes string copies from ownership lookup. The profile at `7083b6e` measured owner lookup at 0.672 ms CPU p95 and 1.057 ms renderer p95 per watch input. Namespace, name and owner-reference keys borrow the immutable input strings. The matching rules remain namespace plus name, last duplicate wins, direct evidence wins and cycles stop traversal. No cache state or new lifetime token is introduced. Clippy and all 30 library and 85 native tests pass; one existing manual benchmark is ignored. Its pre-commit checks pass.

The watch remeasurement passes its single-run gate: CPU p95 7.370 ms, renderer p95 and worst gated p95 7.577 ms, with all 240 measured inputs effective. Artifact: `e2e/artifacts/gpui-frames-2026-11-06T12-22-32-719Z`. Binary SHA-256: `eecba377373cd75dc7c71589bea42bbcf46d9a60d0142ddfd0a32f07aeaa77b7`, from the patch subsequently committed as `4d69cf0`, with the unchanged worktree target/build paths. Uptime: `13:24:13 up 3:20, 1 user, load average: 3.44, 2.69, 2.23`; ending load `[1.75, 2.17, 2.12]`. Both locks and the guard were active. This remains a diagnostic remeasurement, not three-run acceptance.

Commit `24d19ac` adds opt-in spans for table projection, UI construction, live reconciliation, memory preference saves and fixture detail construction. Both default and `frame-profile` Clippy checks pass. The parity contract passes all five tests and 20 assertions. The expanded focused parity run completed successfully under both locks in `e2e/artifacts/parity-d60a9321-5df9-421f-8396-120df81dc36b`. It includes the original required groups, resource filters, kind selection, network flow, metrics, map coverage and custom-resource discovery. No failed scenario required rerun.

The fixture's preference store is memory-only: its Serde-skipped Store uses Default, and neither fixture loading nor frame setup selects a disk directory. Thus real production `sync_all` and rename costs are not measured by these cases. They remain a production-path limitation; they cannot explain a frame-harness failure.

The next component profile completed all 18 non-watch baseline failures at `24d19ac`: `e2e/artifacts/gpui-frames-profile-24d19ac`, binary SHA-256 `81882ceb8513f15c0b5b24faa60ec21587a19f90264cab44e7571c6d9866f73a`. Uptime: `13:36:13 up 3:32, 1 user, load average: 3.89, 3.23, 2.62`. Both locks and the guard were active. It uses the same 120 warm-up and 240 measured inputs in both replays. Table projection takes about 0.1 ms per draw, so it was rejected as an optimization target. In resource opening, memory preference saves take 0.015 ms and fixture detail construction 0.006 ms at CPU p95. Live reconciliation takes 0.012 ms per resource redraw and 0.362 ms per kind-toggle input. These do not account for most Inspector construction cost.

A further single-case profile adds a span around map focus synchronization. Artifact: `e2e/artifacts/gpui-frames-profile-focus`, binary SHA-256 `531d1c48637655fb9f674204d7dda8f10463a0a3d3caa96d8355126c16947ec1`, source `24d19ac` plus that span. Uptime: `13:49:04 up 3:45, 1 user, load average: 4.04, 2.54, 2.41`. Map focus synchronization takes 1.344 ms CPU p95 and 1.707 ms renderer p95 on each unchanged resource redraw. It regenerates every node ID and checks both focus maps. This accounts for most of the otherwise unaccounted Inspector construction time.

Commit `5be5d9a` remembers the immutable view through a Weak pointer and reuses focus registration while that exact view is current. Changed or absent views still run the original reconciliation, including handle cleanup and zoom/skip setup. Only this function mutates the node focus maps. The weak token neither retains old views nor lets allocator reuse validate a stale view. Default and profiling Clippy, all 30 library and 85 native tests, and the 41-scenario parity run pass. The single-run eight-case trial is `e2e/artifacts/gpui-frames-2026-11-06T12-54-14-633Z`, binary SHA-256 `c14cbd5bddb5832c4cdbb03dc94cb2aa26eb5220abeb23553c99d3c114eeb6ca`. Its source is the focus-cache patch before extraction to `map/focus.rs`. Uptime: `13:55:55 up 3:52, 1 user, load average: 3.93, 2.90, 2.52`; load before `[3.94, 2.91, 2.53]`, after `[2.53, 2.65, 2.49]`. Resources, pan, zoom and kind typing passed this trial; detail open, detail close, search typing and kind toggling did not. The final three-run table supersedes this diagnostic.

## Baseline failure witnesses and component profile

The following rows identify the actual nearest-rank p95 frame from the worst gated baseline series. These are timings from that same frame, not sums of independently calculated percentiles. Runqueue is measured over its whole input and cannot be assigned to a particular frame phase when an input has multiple draws. The per-case component profile is a separate instrumented run at `7083b6e` and is diagnostic only. The baseline witnesses agree with every reported worst p95. Layout and prepaint are measured pipeline stages; where a deeper algorithm has not been isolated, that remains an explicit diagnosis limit. The outside-draw column is CPU frame time minus SDK draw time, including updates and post-draw callback/submission work.

| Case | Baseline run and series | Samples | P95 frame | Request/layout | Prepaint | Paint | Outside draw | Whole-input runqueue |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| resources | 3, renderer, content | 240 | 9.127 | 3.230 | 3.293 | 1.676 | 0.538 | 0.005 |
| table-scroll | 3, renderer, content | 240 | 8.678 | 2.981 | 3.299 | 1.538 | 0.491 | 0.000 |
| detail-open | 3, renderer, worst-input | 240 | 13.753 | 4.428 | 4.513 | 2.502 | 1.539 | 0.000 |
| detail-tabs | 3, renderer, content | 240 | 8.333 | 3.559 | 2.401 | 1.064 | 0.968 | 0.000 |
| search-typing | 3, renderer, content | 240 | 15.313 | 3.307 | 3.334 | 1.595 | 6.735 | 0.000 |
| event-filters | 2, renderer, content | 241 | 12.629 | 2.884 | 5.648 | 2.224 | 1.123 | 0.178 |
| map-pan | 3, renderer, content | 240 | 8.543 | 2.897 | 3.080 | 1.673 | 0.522 | 0.000 |
| map-zoom | 2, renderer, content | 241 | 14.366 | 4.543 | 6.212 | 2.116 | 0.876 | 0.779 |
| detail-close | 2, renderer, worst-input | 240 | 10.237 | 4.239 | 3.221 | 1.407 | 1.034 | 0.004 |
| live-limited | 2, renderer, content | 240 | 8.547 | 2.957 | 3.302 | 1.503 | 0.452 | 0.000 |
| live-failed | 3, renderer, content | 240 | 8.887 | 2.328 | 3.869 | 1.747 | 0.526 | 0.531 |
| live-reconnecting | 3, renderer, content | 240 | 8.930 | 2.694 | 3.848 | 1.572 | 0.474 | 0.011 |
| kind-picker | 2, renderer, content | 240 | 8.844 | 3.501 | 2.978 | 1.557 | 0.450 | 0.000 |
| live-watch-burst | 2, renderer, content | 247 | 43.174 | 38.314 | 2.071 | 0.709 | 1.879 | 5.853 |
| exec-selection-drag | 2, CPU, content | 241 | 9.219 | 1.235 | 4.720 | 2.577 | 0.348 | 3.392 |
| kind-picker-toggle | 2, CPU, worst-input | 240 | 19.578 | 5.594 | 4.240 | 1.910 | 7.193 | 0.109 |
| kind-picker-typing | 2, renderer, content | 241 | 9.183 | 3.304 | 3.281 | 1.676 | 0.526 | 0.000 |
| network-flow-pan | 2, renderer, content | 240 | 8.465 | 1.631 | 3.702 | 2.294 | 0.430 | 0.006 |
| gitops-application-open | 1, renderer, content | 240 | 8.704 | 0.761 | 3.453 | 1.128 | 3.121 | 0.000 |

Component costs below are p95 of the sum of each named span within one input, over 240 inputs. Nested spans overlap. CPU and renderer replays are separate. Calls per input are means over both replays. These values must not be added to independent frame percentiles.

| Case | CPU frame p95 | Renderer frame p95 | Largest measured component, CPU / renderer | Calls/input |
| --- | ---: | ---: | --- | ---: |
| resources | 7.240 | 8.025 | inspector.render: 1.988 / 2.191 | 1.000 |
| table-scroll | 7.366 | 8.638 | inspector.render: 1.906 / 2.292 | 1.000 |
| detail-open | 8.720 | 9.313 | inspector.render: 2.733 / 2.838 | 2.002 |
| detail-tabs | 7.264 | 8.191 | detail.request_layout: 2.432 / 2.449 | 1.002 |
| search-typing | 13.094 | 13.848 | rows.filter: 4.050 / 4.118 | 1.000 |
| event-filters | 5.433 | 6.392 | inspector.render: 0.768 / 0.881 | 1.000 |
| map-pan | 7.037 | 7.737 | inspector.render: 1.901 / 2.137 | 1.002 |
| map-zoom | 7.200 | 8.283 | inspector.render: 1.888 / 2.234 | 1.000 |
| detail-close | 8.543 | 8.743 | inspector.render: 5.084 / 4.958 | 2.002 |
| live-limited | 6.766 | 7.769 | inspector.render: 1.869 / 2.242 | 1.000 |
| live-failed | 6.820 | 7.460 | inspector.render: 1.876 / 2.037 | 1.000 |
| live-reconnecting | 8.005 | 7.435 | inspector.render: 2.386 / 2.025 | 1.000 |
| kind-picker | 7.067 | 8.242 | inspector.render: 1.898 / 2.207 | 1.000 |
| live-watch-burst | 10.714 | 10.823 | inspector.render: 5.881 / 5.948 | 1.000 |
| exec-selection-drag | 4.857 | 4.691 | sidebar.paint: 0.685 / 0.587 | 1.010 |
| kind-picker-toggle | 13.111 | 13.872 | inspector.render: 6.737 / 6.967 | 3.021 |
| kind-picker-typing | 7.630 | 8.119 | inspector.render: 2.083 / 2.223 | 1.004 |
| network-flow-pan | 7.143 | 7.408 | page.paint: 1.429 / 1.365 | 1.002 |
| gitops-application-open | 8.299 | 8.178 | page.prepaint: 0.999 / 0.965 | 1.004 |

## Outcome and limits

The requested port is committed on `perf/gpui-frame-time`, with functional source at `5be5d9a`. Required checks and 41 scenarios per parity driver pass. The final optimized gate fails seven cases, each with measured causes above. Twelve original failures pass after optimization. The full 108-case baseline remains available; 86 earlier passes were not remeasured after these changes.

Temporary diagnostic changes were removed. `git diff --exit-code -- src-tauri/gpui` confirms the source is identical to the gated commit. No further functional change follows that measurement. This report and `frames-first-run.md` are committed separately.

These results cover Linux headless CPU frame work and renderer encoding/submission. Physical display presentation, other operating systems and production disk-backed preference writes remain unmeasured. The profiles identify the YAML layout component and Argo completion task; their internal subroutine costs remain unsplit.
