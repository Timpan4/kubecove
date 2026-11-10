# First full GPUI frame repetition

This is repetition 1 of the required three-run protocol. All 108 cases completed, including 57 views and 51 loops. Each replay used 120 warm-up and 240 measured inputs. Every loop had 240 effective inputs. Both existing locks were held, and the Linux isolation guard detected no parity overlap.

Artifact: `e2e/artifacts/gpui-frames-2026-11-06T10-34-20-723Z/run-1.json`. Measured Rust source: `5806142`. Binary SHA-256: `ebc8eeb3c0c9f48c16f1ff798db04e345b541487aa39c0444a0826a131d8f5cf`.

Build: `src-tauri/gpui/build`. Target: `src-tauri/gpui/target`. Optimized, profiling disabled, debug assertions disabled.

Start uptime: `11:34:20 up 1:30, 1 user, load average: 9.93, 16.33, 14.96`. Repetition 2 began at `11:54:14 up 1:50, 1 user, load average: 1.85, 2.11, 5.43`.

99 cases meet the ceiling in this repetition. 9 exceed it. Three-run acceptance remains pending. Times are milliseconds. Worst p95 includes both replays, presentation-only callbacks, worst frames per input and each content-frame ordinal.

| Case | Kind | CPU p95 | Renderer CPU p95 | Worst p95 | Result |
| --- | --- | ---: | ---: | ---: | --- |
| overview | view | 3.052 | 3.409 | 3.409 | Pass |
| resources | view | 6.640 | 7.991 | 7.991 | Pass |
| detail | view | 5.200 | 6.620 | 6.620 | Pass |
| yaml | view | 4.341 | 5.270 | 5.270 | Pass |
| events | view | 4.823 | 5.765 | 5.765 | Pass |
| logs | view | 4.578 | 5.302 | 5.302 | Pass |
| revisions | view | 4.579 | 4.901 | 4.901 | Pass |
| actions | view | 4.311 | 5.307 | 5.307 | Pass |
| cluster-overview | view | 4.273 | 4.896 | 4.896 | Pass |
| incidents | view | 3.223 | 3.463 | 3.463 | Pass |
| port-forwards | view | 4.325 | 4.516 | 4.516 | Pass |
| table-scroll | interaction | 6.989 | 8.220 | 8.220 | Pass |
| log-scroll | interaction | 4.771 | 6.005 | 6.005 | Pass |
| detail-open | interaction | 8.390 | 9.482 | 9.791 | Over |
| detail-tabs | interaction | 7.919 | 8.194 | 8.194 | Pass |
| search-typing | interaction | 12.694 | 14.857 | 14.857 | Over |
| log-filter-typing | interaction | 5.164 | 5.965 | 5.965 | Pass |
| event-filters | interaction | 5.596 | 6.315 | 6.315 | Pass |
| incident-filters | interaction | 3.912 | 4.496 | 4.496 | Pass |
| map-pan | interaction | 6.753 | 8.081 | 8.081 | Pass |
| map-zoom | interaction | 7.038 | 8.452 | 8.452 | Over |
| exec-session-scroll | interaction | 5.091 | 5.838 | 5.838 | Pass |
| detail-close | interaction | 8.219 | 8.718 | 8.936 | Over |
| timeline-toggle-all | interaction | 5.414 | 6.458 | 6.458 | Pass |
| events-search-typing | interaction | 5.155 | 6.144 | 6.144 | Pass |
| log-wrap | interaction | 5.193 | 5.971 | 5.971 | Pass |
| log-order | interaction | 5.400 | 5.867 | 5.867 | Pass |
| forward-start | view | 3.797 | 4.503 | 4.503 | Pass |
| forward-active | view | 3.915 | 4.653 | 4.653 | Pass |
| exec-session | view | 3.767 | 4.593 | 4.593 | Pass |
| live-limited | view | 6.777 | 7.453 | 7.453 | Pass |
| live-failed | view | 6.910 | 7.485 | 7.485 | Pass |
| live-reconnecting | view | 6.408 | 7.982 | 7.982 | Pass |
| custom-resources | view | 4.254 | 4.454 | 4.454 | Pass |
| network-flow | view | 6.407 | 7.436 | 7.436 | Pass |
| metrics | view | 5.536 | 6.431 | 6.431 | Pass |
| usage-footer | view | 2.867 | 3.307 | 3.307 | Pass |
| actions-image | view | 4.129 | 5.528 | 5.528 | Pass |
| actions-environment | view | 5.051 | 5.682 | 5.682 | Pass |
| actions-resources | view | 4.679 | 6.348 | 6.348 | Pass |
| actions-labels | view | 4.806 | 5.447 | 5.447 | Pass |
| actions-annotations | view | 4.293 | 5.525 | 5.525 | Pass |
| actions-rollback | view | 4.270 | 5.379 | 5.379 | Pass |
| yaml-edit | view | 4.894 | 5.894 | 5.894 | Pass |
| yaml-kyaml | view | 4.392 | 5.834 | 5.834 | Pass |
| yaml-edit-kyaml | view | 4.417 | 4.885 | 4.885 | Pass |
| rbac | view | 4.002 | 4.578 | 4.578 | Pass |
| workspaces | view | 1.338 | 1.778 | 1.778 | Pass |
| settings | view | 3.028 | 3.599 | 3.599 | Pass |
| palette | view | 4.466 | 4.875 | 4.875 | Pass |
| gitops | view | 3.758 | 4.651 | 4.651 | Pass |
| gitops-list | view | 3.283 | 3.760 | 3.760 | Pass |
| flux | view | 3.033 | 3.546 | 3.546 | Pass |
| argo-app | view | 4.255 | 4.672 | 4.672 | Pass |
| argo-workspace | view | 4.416 | 6.275 | 6.275 | Pass |
| helm | view | 3.029 | 3.664 | 3.664 | Pass |
| helm-list | view | 3.092 | 3.325 | 3.325 | Pass |
| helm-release | view | 3.276 | 3.658 | 3.658 | Pass |
| settings-agents | view | 2.627 | 3.012 | 3.012 | Pass |
| settings-appearance | view | 3.009 | 3.348 | 3.348 | Pass |
| settings-sessions | view | 2.639 | 2.807 | 2.807 | Pass |
| settings-yaml | view | 2.662 | 2.967 | 2.967 | Pass |
| settings-diagnostics | view | 2.902 | 3.193 | 3.193 | Pass |
| settings-advanced | view | 2.513 | 3.042 | 3.042 | Pass |
| settings-remote | view | 2.527 | 2.814 | 2.814 | Pass |
| settings-updates | view | 2.444 | 2.743 | 2.743 | Pass |
| settings-kubeconfig | view | 2.789 | 3.405 | 3.405 | Pass |
| argo-connected | view | 4.616 | 5.047 | 5.047 | Pass |
| argo-review | view | 4.624 | 5.167 | 5.167 | Pass |
| argo-desired | view | 4.950 | 5.354 | 5.354 | Pass |
| workspace-editor | view | 1.530 | 1.632 | 1.632 | Pass |
| custom-search | view | 3.468 | 3.927 | 3.927 | Pass |
| kind-picker | view | 6.851 | 8.682 | 8.682 | Over |
| live-watch-burst | interaction | 28.347 | 31.371 | 31.371 | Over |
| exec-output-burst | interaction | 5.024 | 5.724 | 5.724 | Pass |
| exec-selection-drag | interaction | 4.048 | 4.417 | 4.417 | Pass |
| helm-search-typing | interaction | 3.083 | 3.290 | 3.290 | Pass |
| kind-picker-toggle | interaction | 13.366 | 15.134 | 16.072 | Over |
| gitops-view-toggle | interaction | 3.866 | 4.417 | 4.417 | Pass |
| helm-view-toggle | interaction | 3.158 | 3.858 | 3.858 | Pass |
| palette-typing | interaction | 4.805 | 5.270 | 5.270 | Pass |
| kind-picker-typing | interaction | 7.206 | 8.376 | 8.376 | Over |
| custom-search-typing | interaction | 3.194 | 3.622 | 3.622 | Pass |
| settings-search-typing | interaction | 6.078 | 6.507 | 6.507 | Pass |
| network-flow-pan | interaction | 6.474 | 7.424 | 7.424 | Pass |
| network-flow-zoom | interaction | 4.553 | 5.268 | 5.268 | Pass |
| metrics-pan | interaction | 5.675 | 7.149 | 7.149 | Pass |
| metrics-zoom | interaction | 4.190 | 5.394 | 5.394 | Pass |
| events-scroll | interaction | 5.224 | 6.683 | 6.683 | Pass |
| timeline-scroll | interaction | 5.894 | 7.120 | 7.120 | Pass |
| workspace-editor-open | interaction | 1.447 | 1.636 | 1.636 | Pass |
| forward-start-typing | interaction | 3.949 | 4.916 | 4.916 | Pass |
| forward-start-submit | interaction | 4.538 | 4.857 | 4.857 | Pass |
| forward-stop | interaction | 4.527 | 4.918 | 4.918 | Pass |
| helm-release-open | interaction | 4.776 | 5.093 | 5.093 | Pass |
| workspace-editor-typing | interaction | 1.598 | 1.725 | 1.725 | Pass |
| rbac-typing | interaction | 4.780 | 5.094 | 5.094 | Pass |
| settings-page-navigation | interaction | 3.345 | 3.764 | 3.764 | Pass |
| argo-document-navigation | interaction | 4.607 | 4.904 | 4.904 | Pass |
| argo-review-open | interaction | 4.811 | 4.767 | 4.811 | Pass |
| flux-view-toggle | interaction | 3.071 | 3.424 | 3.424 | Pass |
| gitops-application-open | interaction | 7.922 | 8.704 | 8.704 | Over |
| network-flow-selection | interaction | 5.666 | 5.915 | 5.991 | Pass |
| metrics-selection | interaction | 6.541 | 6.744 | 6.870 | Pass |
| settings-diagnostics-toggle | interaction | 4.629 | 5.125 | 5.125 | Pass |
| settings-agents-toggle | interaction | 2.879 | 3.026 | 3.026 | Pass |
| settings-agents-port-typing | interaction | 2.777 | 3.133 | 3.133 | Pass |
| rbac-category-navigation | interaction | 4.525 | 4.992 | 4.992 | Pass |
