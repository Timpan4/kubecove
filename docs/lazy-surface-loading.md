# Deferred Svelte surfaces

Issue [#415](https://github.com/Timpan4/kubecove/issues/415) keeps the launcher eager and loads the workspace, resource inspector, GitOps, Helm, RBAC, incidents, live sessions, and settings through dynamic imports. YAML still loads with the inspector. Exec and Ownership Map retain their existing dynamic imports.

Each deferred area shows a loading status and an error with a reload action. Reloading retries the module through a fresh document, including failures cached by the browser module map. Workspace creation persists the selected workspace before that reload. Existing surface keys, props, bindings, and navigation snapshots are preserved.

## Production bundle comparison

Both builds use Vite 8.3.1 on Bun 1.4.2, the same lockfile and release-profile build configuration, and the launcher entry path. The table uses the bundle reports embedded in the native reports linked below, including the existing profiling instrumentation. The baseline is `3aa22398d02a056946651ae120d19517c7eff69a`; the comparison changes only the import graph and deferred rendering.

Eager JavaScript is the transitive closure of static imports from the entry chunk. Compression is measured per file by the existing bundle reporter, not as one concatenated stream.

| Eager JavaScript | Before | After |
| --- | ---: | ---: |
| Raw bytes | 1,579,932 | 539,300 |
| Gzip bytes | 442,060 | 151,521 |
| Brotli bytes | 360,868 | 130,753 |

The workspace static graph no longer contains the inactive screen components or resource inspector. Opening the inspector does not load the live-sessions manager. Screen helpers use their existing focused modules so surface re-exports cannot pull inactive UI into the graph.

## Native launcher comparison

Both native runs use `bun run perf:startup`, release mode with the existing E2E feature, WebKitGTK 2.52.6, and the `launcher-fresh-profile` scenario on Linux x64. Weston 14.0.2 supplies a headless Wayland display with its Pixman renderer. Both runs reach launcher-ready, and each reads kubeconfig sources once.

| Milestone, milliseconds after navigation | Before | After |
| --- | ---: | ---: |
| Frontend entry | 66 | 97 |
| Svelte mount | 118 | 216 |
| Path restored | 135 | 252 |
| Kubeconfig ready | 252 | 422 |
| Launcher ready | 264 | 468 |

These are individual observations, not a startup speed claim. The second run overlapped a separate Rust regression-test build, and OS caches were not flushed. The demonstrated improvement is the smaller eager graph. The native scenario has no live cluster and does not measure first rows or first-open latency. WebKitGTK does not expose `performance.memory`, so JS heap size is unavailable.

The existing Linux usage sampler includes thread tasks and duplicates shared RSS. Its process-tree memory totals are invalid for this comparison and are excluded pending the separate sampler fix. This report does not interpret those totals as retained application memory.

Raw native reports remain at:

- Before: `/tmp/kubecove-lazy-baseline/e2e/artifacts/desktop-1790713803667-832655-f7db3f93/startup.json`.
- After: `e2e/artifacts/desktop-1790713895085-835991-987e314e/startup.json`.

To reproduce without Xvfb, start `weston --backend=headless --renderer=pixman --socket=wayland-kubecove --no-config --idle-time=0` with a private `XDG_RUNTIME_DIR`. Run the existing profiling command in the D-Bus/keyring session described in [Development Workflow](development-workflow.md), with that runtime directory, `WAYLAND_DISPLAY=wayland-kubecove`, and `GDK_BACKEND=wayland`. The profiler requires idle application and development-server ports.

## Behavior verification

The fast E2E suite passes in `e2e/artifacts/fast-1790713703721-828236-f2252ee6/`. Its new spec checks launcher/workspace/inspector request boundaries, inactive screen absence after inspection, YAML restoration after reload, blocked workspace-chunk recovery, and navigation to each deferred area. Screenshots of those areas were inspected. Existing navigation and transition tests pass. Keyboard and focus handlers are unchanged; their preservation was reviewed in source rather than exercised by the mouse-driven E2E spec.

TypeScript, Svelte check, lint, and production build pass. Lint retains the existing five CSS warnings. Native WebKit launcher smoke passes for both builds; macOS and Windows native behavior was not exercised on this host.
