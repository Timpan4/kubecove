# Legacy code and test audit

Audit baseline: `e06e4d3`, 2026-10-17. The cleanup preserves active application behavior, removes only proven obsolete paths, and repairs failures instead of suppressing them.

## Evidence and scope

The frontend pass read all 421 TypeScript/Svelte source files, including tests and declarations, and built a static/dynamic import graph rooted at `src/main.ts` and the diagnostics topology HTML entry. Repository-wide searches checked candidate symbols against tests, benchmarks, scripts, E2E, documentation, and configuration. No variable imports or glob imports introduce additional frontend roots.

Separate subagent passes inspected Rust module registration, workspace/server isolation, platform and benchmark features, and test tooling. Those passes establish evidence for the changes below, not a claim that every possible dead Rust symbol has been eliminated. A final independent diff review found no necessary findings.

## Removed paths

| Cleanup | Obsolescence proof | Coverage retained |
| --- | --- | --- |
| `features/argo/status-insights.ts` and its dedicated test | Its former `ArgoApplicationDetail.tsx` caller was deleted in `b48dbca`. Only its own two tests imported it. Current inspection uses `ArgoApplicationDetails.svelte` and typed inspector snapshots. | Active Argo inspection, operation, workspace, and resource tests remain. |
| `features/gitops/gitops-overview-helpers.ts` and its dedicated test | Only its own four tests called it. The current surface uses `surfaceModel.ts` for selections, provider rail counts, and defaults. | `svelte-gitops-surface-model.test.ts`, GitOps state tests, and navigation tests remain. |
| App-update, GitOps, and Helm `index.ts` barrels | No production import reaches these barrels. The sole GitOps test consumer now imports the same function directly from `surfaceSelection.ts`. | Existing update, selection, and Helm tests remain. |
| `command-palette/store.ts` and `gitops/flux-kinds.ts` | No callers. The Svelte palette owns its bound open state; active GitOps navigation and health presentation use their current modules. | Palette shortcut/search and GitOps navigation tests remain. |
| Legacy panel CSS constants, metadata formatter, old table-state factory/types, kind-ranking table, unused navigation constructors, snapshot getters, and small display helpers | Exact symbol searches found no runtime, test, benchmark, or tooling callers. Active state models, parsers, and validation stay in place. | Existing resource, navigation, incident, Helm, RBAC, diagnostics, and store tests remain. |
| `formatUsageMetricsBreakdown` | Only a test used this label-prefixing wrapper. `AppUsageFooter.svelte` renders labels and metrics separately using the CPU, memory, and process-count formatters. | The nested-row assertions now call the retained, test-only `formatUsageMetricsBreakdownDetails` helper directly; label/depth assertions and production formatter tests remain. |
| Rust `process_tree_pids_from_relations` | A forwarding wrapper with two test-only callers and a production dead-code allowance. Production and benchmarks use `process_tree_pids_from_roots`. | Both single-root regression tests now call the production traversal directly. Multi-root coverage remains. |

The six removed tests passed before removal. They exercised only the two retired modules. No failing test was deleted.

## Repaired failures

- Four existing test cases assumed LF line endings in source/workflow text. Their inputs now normalize CRLF before comparison. Assertions and workflow gates are unchanged.
- Malformed escapes in a workspace URL hash threw `URIError` during startup. A regression test first reproduced the failure for both workspace ID and view mode. `parsePathStateHash` now rejects either malformed segment, allowing the existing saved-session fallback to run. No persistence schema changes.
- The copy-control server-render test timed out during the busy baseline run, then passed in isolation and in the subsequent full suite. Its assertions and timeout are unchanged.

## Retained after review

Compatibility/storage migrations, protected Bits UI wrappers, supported Tauri command APIs, test dependency controls, benchmark entrypoints, the diagnostics topology harness, and environment/platform-gated E2E tests remain. A missing production caller alone did not prove the remaining tested helpers obsolete.

## Verification

Local logs and browser evidence are under ignored `e2e/artifacts/legacy-audit/`. The browser check uses fake development data and an isolated browser profile. It verifies startup from a malformed workspace hash and recovery of saved Settings from a malformed view hash. The user-launched desktop app and existing development servers remain running.

| Check | Result |
| --- | --- |
| `bun test` with native Git Bash on the process PATH | 754 passed, 3 existing platform skips, 0 failed across 104 files. |
| `bun run build` | Frontend typecheck and production bundle passed. Existing chunk-size warning remains. |
| `bun run typecheck:bench` | Passed. |
| `bun run svelte:check` | 0 errors, 0 warnings, using the existing script exclusions. |
| `bun run lint` | Passed with 8 existing warnings and 2 informational notices in unchanged code. Biome formatting is disabled by repository configuration. |
| `bun run lint:anti-slop` | Passed. |
| `cargo check --locked --workspace --manifest-path src-tauri/Cargo.toml` | Passed on Windows. |
| `cargo test --locked --manifest-path src-tauri/Cargo.toml -p kubecove-host` | 13 passed, 0 failed. |
| `cargo clippy --locked --manifest-path src-tauri/Cargo.toml -p kubecove-host --all-targets -- -D warnings` | Passed. |
| `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check` | Passed. |
| `bun run docs:check` and `git diff --check` | Passed. |
| Collaborative browser | Malformed hashes rejected, saved Settings restored and rendered without console errors. |

On this Windows host, `bash` resolves to the WSL launcher. Workflow regression tests need the installed native Git Bash so Windows temporary paths and environment variables reach the shell. For this process only:

```powershell
$env:Path = 'C:\Program Files\Git\bin;' + $env:Path
bun test
```

This audit does not establish native desktop, live-cluster, headless-server E2E, or macOS/Linux runtime behavior. No cluster was created or changed. Existing lint and bundle-size warnings remain outside the cleanup.
