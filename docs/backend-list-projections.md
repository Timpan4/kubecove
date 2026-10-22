# Backend list projection batch

Contract: preserve serialized summaries, ordering, RBAC and error classification,
cancellation and failed-reload restoration while reducing transient allocations.

## Failure modes recorded before implementation

1. Projection drops a summary, health, ownership, timestamp or topology field.
   Compare serialized outputs against full Kubernetes decoding at a fixed clock.
2. Missing or null metadata/status/spec changes generated Kubernetes defaults.
   Compare empty, null and populated fixtures, including zero and negative counts.
3. Filtering annotations drops Argo, Flux or Helm ownership. Keep all annotations
   except last-applied configuration, which no summary consumer reads.
4. Typed resource identity or retained fields lose validation. Keep typed identity
   validation and serde field types. Fields omitted by the projection are parsed
   as valid JSON but do not receive generated-model validation. Some retained
   required fields also default when missing, as documented below.
5. Dynamic topology loses labels, uid or owner references. Compare topology inputs
   as well as summaries; retain the complete status value for arbitrary CRs.
6. Bounded fetches reorder results/errors or leave cache loads undriven after an
   error. Preserve input order and drive every fetch to completion before
   inspecting errors; release network permits as individual fetches finish.
7. Cache pruning removes loading entries, the requested stale value before a
   reload, externally held values, or a previous value restored after failure.
   Exercise those states with controlled timestamps, without sleeps.
8. Client cache overflow drops unrelated clients or evicts a replacement entry.
   Fill the existing capacity, replace a key, then overflow and check survivors.
9. Helm labels disagree with decoded release identity/version, decoding fails, or
   list permission exists without get permission. Labels cannot safely select
   winners under the unchanged-output contract. Preserve list-only access and
   decoded precedence; omit manifests/values only from list record construction.

## Delivered scope

1. Secret and ConfigMap summaries use `list_metadata`. Neither summary includes
   data or key counts. Detail and disclosure paths remain complete.
2. Pod, Deployment, ReplicaSet and Job summary lists decode narrow typed
   projections. Event lists for details and incidents also use a projection.
   Existing summary builders retain their defaults and health rules. Fields
   omitted by projections receive JSON syntax validation, without generated-model
   validation. The projections also accept some malformed objects that full
   decoding rejects: missing PodCondition `type`/`status`, ContainerStatus
   `restartCount`, ReplicaSetStatus `replicas`, or Event `involvedObject` now use
   defaults. Null metadata becomes empty metadata. A non-string last-applied
   annotation is discarded along with that annotation instead of failing the
   list. These differences do not affect conforming Kubernetes API servers.
3. Dynamic summary and topology lists retain slim metadata and complete status.
   Owner references, labels and uid remain available. Managed fields, spec and
   last-applied configuration are skipped during decoding. Detail/get paths
   remain complete.
4. Scope lists share a network concurrency bound of 16 across kinds and
   namespaces, matching topology's existing ceiling. Cache hits and shared-load
   joins do not take permits. Warning Event and restart evidence lists also use
   semaphore-bounded fetches. `join_all` drives every fetch to completion and
   retains results in input order before error handling, as in the base. Finished
   fetches release permits immediately, even while an earlier fetch is pending.
   Cancellation still drops the joined futures without spawning detached work;
   workspace cancellation still signals and removes shared cache loads.
5. Scope responses retain cached batches through serialization. Sorted row
   references preserve last-duplicate precedence; only rows whose Flux ownership
   fields change are copied. The public JSON and TypeScript model are unchanged.
   The incident caller still receives owned rows because it needs that form.
6. Helm lists decode release identity/chart summaries without retaining values,
   manifests, or parsed manifest resources per revision. Detail and reconciliation
   records still decode complete payloads. Metadata-first winner selection was
   rejected because decoded identity/version override labels, and adding GETs
   breaks list-only RBAC. Namespace fallback, ties and decode-error fallback are
   preserved. All revisions still need fetching and decompression.
7. Cache activity prunes unheld ready entries older than the existing longest
   freshness window, 30 seconds. Requested stale values remain available for
   failed reloads. Loading and externally held values are preserved. This is
   lazy pruning, not an idle timer. Watch invalidation formats its prefix once.
8. Client cache overflow evicts the least recently used entry; successful lookups
   and replacements refresh its position. Fingerprint misses do not. Generation
   rotation still clears both entries and recency and cancels the old
   generation.
9. Pagination is deferred. Setting a global limit alone truncates existing
   callers. Continuations, snapshot consistency and expiration/error handling
   need a caller-level change rather than a `list_params` edit.

## Measurements

Native Linux release wall-clock medians. These are synthetic decoder workloads,
not live Kubernetes latency or RSS measurements. Other work was running on the
host; do not interpret the timings as stable application latency. The CodSpeed
CLI is not installed. The cases use the existing CodSpeed-compatible benchmark
target and run in its native wall-clock mode.

The Pod case decodes and builds summaries for 1,000 Pods. Other list cases decode
and drop 1,000 objects; existing summary/topology benchmarks cover post-decode
mapping. Workload specs contain Pod templates with environment variables, managed
fields and last-applied annotations. Events have no spec or last-applied payload.
Both decoders consume identical JSON. The Helm case uses the existing 250-record
fixture and produces the same 51 latest summaries.

| Path | Full/before median | Projected/after median |
| --- | ---: | ---: |
| Pod decode and summaries | 8.368 ms | 2.931 ms |
| Deployment decode | 11.35 ms | 2.461 ms |
| ReplicaSet decode | 9.304 ms | 2.604 ms |
| Job decode | 8.965 ms | 2.611 ms |
| Event decode | 904.7 us | 680.8 us |
| Dynamic Widget decode | 17.92 ms | 2.924 ms |
| Helm records to latest summaries | 3.766 ms | 1.105 ms |
| Cached scope assembly and JSON encoding | 3.220 ms | 1.542 ms |

Baseline Helm and initial Pod comparison:

```sh
cargo bench --manifest-path src-tauri/Cargo.toml --features bench-support --bench backend_hot_paths -- 'pod_list_decode|helm_storage_records_to_latest_releases'
```

Final comparison, including the added decoder variants:

```sh
cargo bench --manifest-path src-tauri/Cargo.toml --features bench-support --bench backend_hot_paths -- 'list_decode|helm_storage_records_to_latest_releases'
```

The initial Pod comparison measured 12.33 ms full and 3.209 ms projected. The table
uses the final paired run; its Helm baseline predates the list-record change.

The cached scope pair uses the existing scope fixture with an empty Flux inventory
and identical output. Cached batches are prepared outside measurement. The owned
case includes the previous cache-hit copies; the shared case serializes borrowed
rows. Rows that need Flux changes retain the ownership-copy path, covered by tests.

```sh
CARGO_BUILD_JOBS=2 nice -n 19 cargo bench --manifest-path src-tauri/Cargo.toml --features bench-support --bench backend_hot_paths -- scope_cached_rows
```

## Verification

All requested checks pass:

```sh
cargo check --workspace --manifest-path src-tauri/Cargo.toml
cargo clippy --workspace --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
cargo test --workspace --manifest-path src-tauri/Cargo.toml
cargo check --manifest-path src-tauri/server/Cargo.toml
```

Workspace tests: 388 passed, two existing ignored tests. Projection tests compare
serialized bytes against full decoding at a fixed clock, including missing/null
fields, ownership, status and health. Dynamic topology compares uid, labels and
owner references too. Helm parity includes conflicting labels, null fields and
invalid manifest/version types. Existing tests cover RBAC fallback and storage
failure reporting. Cache tests cover overflow, replacement, stale eviction,
externally held values and restoration after a failed reload. Shared scope tests
compare full serialized response bytes with the previous owned assembly, check
ordering and duplicate precedence, and prove that unchanged cached rows are not
copied or mutated. Flux tests compare copy admission with actual enrichment.

`cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check`,
`bun run docs:check` and `git diff --check` pass. The default server check initially
failed because this worktree lacked `dist/`; generating it with the documented
`KUBECOVE_PUBLIC_RUNTIME=web bun --bun vite build` resolved that prerequisite.
The API-only server check also passes. Hooks are configured with
`core.hooksPath=.githooks`.

No running app was launched, stopped or exercised. No cluster requests, deployment,
push, live RSS measurement, cross-platform runtime check or CodSpeed instrumented
run was performed. The repository's milestones document now directs tracking to
GitHub Issues and contains no corresponding checklist item to update.
