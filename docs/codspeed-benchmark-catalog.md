# CodSpeed benchmark catalog

Audit baseline: `05975d288b018d8c833ec84ff1ecbcd8f38e9704`. Inventory audited 2026-10-10; benchmark value reviewed against production callers on 2026-10-11. These are justified measurement workloads, not claims of measured bottlenecks. Implementation is complete and locally validated; the audit tables preserve the baseline findings.

The value review accounts for all 108 original candidates. It retains 63 worthwhile workload families, 35 frontend and 28 Rust, folds 34 helpers into those production pipelines and rejects 11 standalone targets. This is the full justified set, not a shortlist. Retained families can contain separate cold, warm, update, presentation or encoding variants. Each has caller/trigger evidence, scaling dimensions, coverage gaps, fixtures, invariants and an optimization question. All frontend source domains and all 133 Rust source files remain accounted for in the audit appendices.

## Implementation status

The 2026-10-21 backend list projection batch adds twelve decoder variants to
`backend_hot_paths::list_projections`: full/projected Pod decoding plus summary
construction, and full/projected decoding for Deployment, ReplicaSet, Job, Event
and dynamic Widget lists. These cover wire decoding that the existing R01/R04
post-fetch summary workloads exclude. Two cached scope assembly/encoding cases
compare the previous owned pipeline with shared-row serialization, using prepared
cached batches and an empty Flux inventory. The existing R25/R26 Helm workload now
uses the production narrow list-record path; its full detail projection checks
remain intact. Native wall-clock comparisons and verification limits are in
[Backend list projections](backend-list-projections.md). These additions have
not been run with the CodSpeed instrument; the instrumented counts below describe
the earlier audit implementation.

The implementation covers all 63 retained families and their 34 folded helpers. All 35 frontend families execute across 25 files, including reruns of corrected fixtures. All 100 registered Rust variants execute successfully in one instrumented CodSpeed run. Independent caller reviews and their fixture corrections are reconciled, including namespace-qualified joins, owner precedence, Secret redaction, cache isolation, and retained-state timing boundaries. Parallel implementation and review used GPT-6 Luna with xhigh reasoning.

CI now enumerates frontend benchmark files into independent jobs. The exact-file selector rejects unknown files; the legacy topology/other selectors remain complete and disjoint. Shared fixtures of any extension select both language lanes. Change detection fetches missing comparison commits without cloning repository history. Rust keeps one benchmark target and caches workspace crates as well as dependencies.

The changes are motivated by existing hosted runs, not projected savings. [Run 37370591185](https://github.com/Timpan4/kubecove/actions/runs/37370591185) spent 4m04s in frontend topology measurement and 2m05s in the other frontend measurement step. [Run 37574919430](https://github.com/Timpan4/kubecove/actions/runs/37574919430) spent 5m52s compiling Rust benchmarks and 27s in its benchmark action. These include runner/setup differences and do not establish a speedup for the new workflow. Hosted timing after this change remains unverified until the branch runs in GitHub Actions.

Local integration evidence: all 25 frontend files executed through the exact-file selector, with coverage corrections rerun successfully. Application and benchmark TypeScript checks, Svelte checks, Biome lint and anti-slop lint pass. The focused frontend/workflow regression selection passes 142 tests; Node's CI/CodSpeed workflow selection passes all 86 tests. Ordinary `cargo check`, all-target/all-feature Clippy with warnings denied, and Rust formatting checks pass. The pinned CI `cargo-codspeed` 5.0.1 build/run completed with exactly 100 unique `Checked:` cases and no panics. These local instrumented runs prove execution and fixture assertions, but do not produce hosted CodSpeed performance measurements. Changed source files meet the repository's hard size limits.

The malformed Helm manifest fixture exposed a production parser loop: the YAML document iterator cannot advance after a syntax error. Parsing now stops at that error and preserves the valid prefix. The regression was written before the fix, and all 13 Helm storage tests pass. Incident regression checks also pass after extracting the production post-fetch seam.

The benchmark work is integrated over `origin/main` at `d714d0b1953d8d0a4e1b1a9c4ec0aa957ff917df`, the v0.12.0 release. The palette cache fixture now carries resource-scope response objects, matching the updated caller. Resource-scope assembly preserves completed requests and permission failures, and shared commands retain the upstream headless runtime boundaries. After integration, all 100 Rust variants and the palette suite execute successfully; 11 palette tests and 92 CI/CodSpeed/release workflow tests pass. Application and benchmark type checks, Svelte checks, lint, desktop/all-feature compilation, headless library compilation, and all-feature Clippy pass. The audit inventory above remains scoped to its recorded baseline; the newly imported headless and remote modules have not been added to that inventory.

## Executable workload map

This map locates the implementation for every retained family and folded helper. Execution and caller review are tracked separately from source presence. Frontend cases use exported production functions or extracted functions that the application now calls. Rust cases share one registered target; supporting modules are not additional Cargo targets.

| Retained frontend families | Benchmark files | Folded coverage |
| --- | --- | --- |
| F01, F02 | [resource-table](../benchmarks/resource-table.bench.ts), [workspace state](../benchmarks/resource-workspace-state.bench.ts) | F03 ownership/selection reconciliation; F29 snapshots |
| F04 | [resource planning](../benchmarks/resource-planning.bench.ts), [sidebar](../benchmarks/sidebar-tree.bench.ts), [palette](../benchmarks/command-palette.bench.ts) | F28 present kinds; F33 workspace planning; F38 provider/workspace scope |
| F05 | [metrics](../benchmarks/resource-metrics.bench.ts) | Shared cold index and retained sparse joins |
| F06 | [table to map](../benchmarks/topology-map.bench.ts) | F03 selection reconciliation |
| F07, F09, F10 | [topology](../benchmarks/topology.bench.ts), [large topology](../benchmarks/topology-large.bench.ts), [layout](../benchmarks/topology-layout.bench.ts) | F08 standalone and network branches |
| F12 | [metadata](../benchmarks/detail-metadata.bench.ts) | Curated and raw detail work |
| F13, F14, F15 | [resource detail](../benchmarks/resource-detail.bench.ts) | F11 incident signals; F24 mounted timestamps |
| F16 | [YAML diff](../benchmarks/yaml-diff.bench.ts) | Active styles and conflict presentation |
| F18 | [Secrets](../benchmarks/yaml-secrets.bench.ts) | Full parse and selected decoding |
| F19 | [formatting](../benchmarks/yaml-format.bench.ts) | Validation followed by YAML/KYAML emission |
| F21, F22 | [editor](../benchmarks/yaml-editor.bench.ts) | F17 returned diagnostic ranges; F23 error lens |
| F26 | [sidebar](../benchmarks/sidebar-tree.bench.ts) | F27 expanded query maps |
| F31, F32 | [workspace health and entries](../benchmarks/workspace-health-entries.bench.ts) | F30 pin/normalization; F35 changed persistence |
| F36, F37 | [palette](../benchmarks/command-palette.bench.ts) | Cache harvest, retained typing and F38 scope |
| F39, F40, F41, F42 | [RBAC](../benchmarks/rbac-surface.bench.ts), [major surfaces](../benchmarks/major-surfaces.bench.ts) | Cold cockpit, observed permissions, reviewed evidence, mounted queues |
| F44, F45 | [major surfaces](../benchmarks/major-surfaces.bench.ts) | F43 Helm grouping; F46 eager GitOps row presentation |
| F47 | [Argo workspace](../benchmarks/argo-workspace.bench.ts) | Managed-resource matching/counts |
| F48 | [Argo resource display](../benchmarks/argo-resource-display.bench.ts) | F49 application YAML fallback |
| F50 | [Argo operation](../benchmarks/argo-operation.bench.ts) | F25 message redaction |
| F51, F52 | [incidents](../benchmarks/incidents.bench.ts) | F53 selected guidance |
| F54 | [diagnostics](../benchmarks/diagnostics.bench.ts) | Snapshot, presentation and report |
| F58 | [sessions](../benchmarks/live-sessions.bench.ts) | F59 saved-forward lookup |

| Rust families and encoding fold | Registered benchmark modules | Folded coverage |
| --- | --- | --- |
| R01, R04, R17 | `resource_catalog::summaries` | R02 metadata, R03 health, Namespace projection and CRD catalog |
| R05, R06, R18 | `resource_catalog::planning` | R07 Flux enrichment; synthetic kubeconfig parsing and validation |
| R08, R09 | `resource_catalog::topology` | R10 production graph input conversion; R07 Flux ownership |
| R11 | `resource_catalog::cache` | R12 keys, R14/R15 deep-cloned reads and invalidation |
| R20, R22, R23 | `resource_catalog::inspection` | R21 detail shaping; R24 schema output mapping |
| R25, R27, R28 | `domain_catalog::domain_helm` | R26 latest release selection; manifest preview within storage records |
| R29, R30, R32 | `domain_catalog::domain_argo` | R31 selected comparisons within Kubernetes/connected inspection |
| R35 | `domain_catalog::domain_flux` | R34 inventory summaries |
| R36 | `domain_catalog::domain_rbac` | R37 raw-rule projection and rollup |
| R39 | `resource_catalog::resource_metrics` | R38 quantities and workload owner rollup |
| R40, R41 | `resource_catalog::incidents` | Post-fetch incident assembly and selected events |
| R42, R43, R44 | `domain_catalog::domain_streams` | Bounded log reads, incremental UTF-8 and source enumeration |
| R45 | `domain_catalog::domain_sessions` | Named/numeric target resolution |
| R47 | `domain_catalog::domain_usage` | Prepared process attribution; no OS sampling |
| R48 | `resource_catalog::encoding` and named `*_json_encoding` cases in domain modules | Complete typed outputs from the retained domain pipelines, including decoded stream messages |

Rust module paths resolve from [backend_hot_paths.rs](../src-tauri/benches/backend_hot_paths.rs), [resource_catalog.rs](../src-tauri/benches/backend_hot_paths/resource_catalog.rs) and [domain_catalog.rs](../src-tauri/benches/domain_catalog.rs). Their feature-gated production bridge starts at [bench_support.rs](../src-tauri/src/commands/bench_support.rs).

## What earns a benchmark

This is a complete workload review rather than a starter shortlist. A retained target must have a production caller, a concrete repeat trigger or substantial document/collection cost, an identifiable allocation or scaling mechanism, and an optimization question its result can settle. Existing useful regression signals also count. A missing test or an available pure function does not establish benchmark value.

"Retain" means an independent production workload is justified. "Fold" means exercise the helper inside its named production workload or an attributable variant. "Remove" means no standalone target is justified by the inspected workload. These decisions govern the fixture/seam notes in the audit tables. They do not request deleting current benchmark code.

Keep complete local pipelines and retained-state interactions distinct. Index construction, parsing and ownership projection on data arrival must not be charged to every search keystroke. Conversely, a full-response clone or serialization cannot be dismissed as a tiny cache lookup or identity check. The workload specifications below state which work belongs in each timed operation.

## Coverage at the audit baseline

There are 39 cases: 30 frontend cases across eight files and nine Rust cases in one target.

| Frontend suite | Cases | Timed work and existing input |
| --- | ---: | --- |
| [Resource table](../benchmarks/resource-table.bench.ts) | 7 | Search-index build/filter; projection; filtering, sorting, grouping and pagination; four state variants; prepared multi-hop ownership rows. 10,000 rows. |
| [Topology](../benchmarks/topology.bench.ts) | 7 | Ownership layout, selection, viewport calculations and retained layout state. 500 or 2,000 Deployment/Pod pairs. |
| [Resource detail](../benchmarks/resource-detail.bench.ts) | 4 | Log append/retention, parse/order/search, event sort/summary/filter, incident timeline. 1,000 input logs/events; append case processes 2,000 entries. |
| [Sidebar tree](../benchmarks/sidebar-tree.bench.ts) | 3 | Eager namespace trees, shallow trees and complete sidebar shaping. 1,000 namespaces and 100 custom kinds. |
| [Major surfaces](../benchmarks/major-surfaces.bench.ts) | 3 | Combined RBAC cockpit, GitOps rails/summaries/tables, and Helm grouping/filtering/sorting. |
| [Command palette](../benchmarks/command-palette.bench.ts) | 2 | Flatten/dedupe/index versus dedupe/index without flattening. Two overlapping sets totaling 10,000 rows and 7,500 unique identities. |
| [Resource metrics](../benchmarks/resource-metrics.bench.ts) | 2 | Table/topology merges with duplicate versus shared index construction. 10,000 matching metrics and rows. |
| [YAML diff](../benchmarks/yaml-diff.bench.ts) | 2 | Compact git-style and full clean-style diffs for changed Deployment manifests containing 40 containers. |

All Rust cases live in [backend_hot_paths.rs](../src-tauri/benches/backend_hot_paths.rs) and call production helpers through the feature-gated [bench-support bridge](../src-tauri/src/commands/bench_support.rs).

| Rust case module | Timed work and existing input |
| --- | --- |
| `helm_manifest_parse_250_resources` | Parse 250 minimal Deployment documents. |
| `helm_manifest_summary_250_resources` | Summarize prepared resources, including the production bounded preview. |
| `serialize_large_resource_yaml` | Emit YAML from prepared JSON containing 40 containers. |
| `serialize_large_resource_kyaml` | Emit KYAML from the same prepared JSON. |
| `build_ownership_topology_500_apps` | Convert/clone inputs and construct a graph from 2,000 resources. |
| `sort_custom_resource_catalog_1000_kinds` | Clone/sort/dedupe 1,100 entries, including 100 duplicates. |
| `present_custom_resource_scope_key_250_namespaces` | Clone/sort/dedupe/join 250 reverse-ordered namespaces. |
| `build_ownership_topology_500_apps_with_crd_overlay` | Clone and construct a graph from 2,500 CRD/workload resources. |
| `inspect_cached_payload_weights_500_apps` | Collect six cache snapshots over two populated ready entries. Fixture initialization is explicitly outside measurement. |

## Coverage gaps in existing fixtures

1. The multi-hop table case prepares `buildResourceTableProjection(ownershipRows)` before measurement. The timed projection case uses another fixture whose Pod owner names do not resolve to its Deployment names. Add a projection measurement over the existing ownership fixture before optimizing inheritance.
2. The small topology rebuild case processes ten selections; indexed selection processes fifty. Compare equivalent work or report per-update cost. The rebuild-view and retained-view cases already use equivalent update counts.
3. The GitOps fixture supplies legacy `healthStatus`, but its summary reads `healthAssessment`. Populate the current assessment contract to measure `healthy`, `degraded`, `needsAttention`, `unknown`, `notEvaluated` and partial evidence.
4. Rust YAML coverage calls `serialize_json_value_document`. It excludes typed-object conversion, normalization and Secret redaction in the production document pipeline.
5. Rust ownership coverage includes conversion/cloning. Keep that pipeline signal, but isolate graph construction when attributing an improvement. Explicitly prepare `LazyLock` fixtures before timing rather than relying on unverified runner warmup behavior.
6. The log-retention case mixes initial fill and steady-state eviction and times its length assertion. Separate those workloads when changing buffering; check expected output outside measurement.
7. Broad surface callbacks combine helpers. Retain the whole-surface case and use a focused case only when needed to identify which stage changes.

## Frontend candidates

The value review retains 35 production workloads, folds 19 helpers into named pipelines and rejects six standalone targets. Decisions govern the original fixture/seam notes. "Ready" means exported TypeScript work is available to Vitest; "Extract" separates substantive computation from a Svelte script. The benchmark configuration has no Svelte compiler plugin. Headless seams were inspected, not executed. Svelte derived work runs when consumed; hidden views do not automatically execute it. Query stale time is not a polling interval.

### Resources

| Candidate and operation | Coverage and access | Fixture and correctness | Value decision and production evidence |
| --- | --- | --- | --- |
| F01. Realistic ownership projection. [resourceTableModel.ts](../src/features/resources/resourceTableModel.ts), [gitops-ownership-evidence.ts](../src/lib/gitops-ownership-evidence.ts). `buildResourceTableProjection; inheritGitOpsOwnership` | Extend. Ready; time projection over the existing multi-hop fixture. | Multi-tier chains, fanout, direct Argo/Flux evidence, ownerless rows, missing parents and cycles. Check identity, ownership precedence, stable order, termination and input immutability. | Retain. [ResourceBrowser.svelte:644](../src/features/resources/ResourceBrowser.svelte) builds ownership/search projection from `tableRows`. Row, metric, kind-scope and Argo-filter changes invalidate it. Text search does not. Time actual ancestor traversal, currently outside the multi-hop benchmark. |
| F02. Retained table interactions. [resourceTableModel.ts](../src/features/resources/resourceTableModel.ts). `buildResourceTableModel` | Extend. Ready; retain the projection as production does. | Ownerless and Flux/Helm groups, preferred Argo identities, selected/collapsed groups, health/metrics sorts and pages. Check counts before health filtering, group conservation, selection visibility and ordering. | Retain. [ResourceBrowser.svelte:658](../src/features/resources/ResourceBrowser.svelte) rebuilds the retained table model on search, filters, sort, page, collapse and selection changes. Full matching inventory is processed before the 50-row page. Preserve existing coverage and distinguish these interactions. |
| F03. Kind filtering and selection reconciliation. [resourceBrowserModel.ts](../src/features/resources/resourceBrowserModel.ts). `filterResourcesByKinds; syncedTopologyNodeId` | New. Ready. | Builtins and versioned CRDs; early, late and missing matches. Check exact dynamic identity and fallback order. | Fold. Kind filtering at [ResourceBrowser.svelte:606](../src/features/resources/ResourceBrowser.svelte) becomes F01's row-preparation fixture. Selection reconciliation at `:722` becomes F06's table-to-map selection fixture. These have different triggers, so do not combine them into one arbitrary batch. |
| F04. Scope, fetch and watch plans. [helpers.ts](../src/features/resources/helpers.ts), [resourceBrowserReadSpecs.ts](../src/features/resources/resourceBrowserReadSpecs.ts), [queryKeys.ts](../src/lib/queryKeys.ts), [ResourceBrowserTopBar.svelte](../src/features/resources/ResourceBrowserTopBar.svelte). `buildFetchKeys; watchKeysFromFetchKeys; mergeWatchKeys; topologyWatchKeys; selector preparation` | New. Exported helpers ready; extract inline selector work only if selected. | Namespace/kind products, cluster/all scopes and overlapping keys. Check canonical identity, first-seen order and deduplication; exclude query execution and request IDs. | Retain. Scope products and canonical read/watch identities at [ResourceBrowser.svelte:325](../src/features/resources/ResourceBrowser.svelte), `:326`, `:838`. Namespace/kind/settings/map changes trigger planning. Selector search at [ResourceScopeSelector.svelte:55](../src/features/resources/ResourceScopeSelector.svelte) is a separate retained-options fixture. |
| F05. Sparse metrics joins. [resource-metrics.ts](../src/lib/resource-metrics.ts). `Metric indexes and table/topology merges` | Extend. Ready. | Sparse, unavailable, unsupported, zero and unmatched identities. Check warnings, edge preservation and immutable inputs. | Retain. Metric index at [ResourceBrowser.svelte:605](../src/features/resources/ResourceBrowser.svelte), table join at `:609`, map join at `:675`. Index rebuilds on metric data; consumers can reuse it. Existing dense cold fixture misses sparse joins and retained-index updates. |

### Topology

| Candidate and operation | Coverage and access | Fixture and correctness | Value decision and production evidence |
| --- | --- | --- | --- |
| F06. Table-to-map updates. [resourceBrowserModel.ts](../src/features/resources/resourceBrowserModel.ts). `filterHistoricalReplicaSets; filterTopologyByTableRows` | New. Ready. | Current/older ReplicaSets, Pod evidence, branched/cyclic graphs, missing edges and empty tables. Check evidence-based hiding, ancestor closure and warnings. | Retain. [ResourceBrowser.svelte:675](../src/features/resources/ResourceBrowser.svelte) runs historical filtering, table ancestry and metric merge together. Consumed table changes can repeat the whole expression, including page/selection changes. Add the actual table-to-map update fixture. |
| F07. Graph normalization and branching layout. [topology-implementation.ts](../src/features/resources/topology-implementation.ts), [topology-graph.ts](../src/features/resources/topology-graph.ts), [topology-layout.ts](../src/features/resources/topology-layout.ts), [topology-view.ts](../src/features/resources/topology-view.ts). `Graph preparation and ownership layout` | Extend. Ready. | CronJob chains, shared timestamps, duplicate IDs and dangling edges. Check finite coordinates, stable parent/order semantics and IDs. | Retain. [OwnershipMap.svelte:68](../src/features/resources/OwnershipMap.svelte) builds layout state on topology, mode, expansion and selected-standalone-expansion changes. Connected-node selection normally uses retained layout. Preserve active layout coverage, add real branching and normalization shapes. |
| F08. Standalone groups and network layout. [topology-standalone-groups.ts](../src/features/resources/topology-standalone-groups.ts), [topology-view.ts](../src/features/resources/topology-view.ts), [topology-implementation.ts](../src/features/resources/topology-implementation.ts). `Standalone grouping and network-mode view layout` | Extend. Ready. | Collapsed/expanded/selected groups, dynamic and cluster-scoped nodes, network relations. Check counts, relative positions and selection preservation. | Fold. Standalone grouping and network layout are branches of the same layout-state caller, [OwnershipMap.svelte:68](../src/features/resources/OwnershipMap.svelte). Measure collapsed/expanded and network modes as named layout fixtures. |
| F09. Retained selection and root focus. [topology-implementation.ts](../src/features/resources/topology-implementation.ts), [topology-graph.ts](../src/features/resources/topology-graph.ts), [topology-view.ts](../src/features/resources/topology-view.ts). `applyFlowTopologySelectionWithIndex; filterFlowTopologyToSelectedRoot` | Extend. Ready; build retained indexes outside interaction timing. | Root, intermediate, leaf, shared, isolated and missing selections. Check equivalent work per update, copied arrays and unchanged positions. | Retain. [OwnershipMap.svelte:78](../src/features/resources/OwnershipMap.svelte) applies selection, root focus and extent to retained layout on selection, focus mode and viewport changes. Existing retained-view cases are production-relevant. |
| F10. Viewport fit planning. [topology-implementation.ts](../src/features/resources/topology-implementation.ts), [OwnershipMapViewport.svelte](../src/features/resources/OwnershipMapViewport.svelte). `buildFlowTopologyFitPlan` | New. Ready; prepare flow nodes and explicit viewport. | Relative children, measured/unmeasured nodes, missing parents, empty graph and resize. Check production zoom bounds, permutation key, geometry and selected-node semantics. | Retain. [OwnershipMapViewport.svelte:32](../src/features/resources/OwnershipMapViewport.svelte) builds the fit plan before the equality guard at `:43`. Node/edge/selection/size/key changes trigger it. Existing topology cases do not time this caller. |

### Details

| Candidate and operation | Coverage and access | Fixture and correctness | Value decision and production evidence |
| --- | --- | --- | --- |
| F11. Conditions, containers and incident signals. [helpers.ts](../src/features/resource-detail/helpers.ts). `getConditionRows; getContainerStatusRows; buildIncidentSignals` | New. Ready; fixed time. | Regular/init/ephemeral containers, malformed status, terminal workloads, restarts and recent/stale warnings. Check fallbacks, status precedence and evidence. | Fold. Conditions, container rows and signals at [ResourceDetailPanel.svelte:321](../src/features/resource-detail/ResourceDetailPanel.svelte), `:322`, `:348` are detail-data preparation for the timeline/signal surface. No reason to invent repeated cold parsing on timeline expansion. |
| F12. Curated metadata and raw formatting. [metadata-details.ts](../src/features/resource-detail/metadata-details.ts), [helpers.ts](../src/features/resource-detail/helpers.ts), [DetailsTab.svelte](../src/features/resource-detail/DetailsTab.svelte). `Curated metadata, badges, annotations and managers; raw sections` | New. Exports ready; extract substantial inline formatting if selected. | Labels, annotations, managedFields and nested status. Check keys/counts, controller/manager order and fallbacks. | Retain. Curated metadata and raw rows at [ResourceDetailPanel.svelte:329](../src/features/resource-detail/ResourceDetailPanel.svelte), `:330`, `:339`. Detail-data changes trigger preparation; badge expansion only changes slicing. Raw metadata inside native `<details>` is still constructed while collapsed, [DetailsTab.svelte:474](../src/features/resource-detail/DetailsTab.svelte), `:487`. |
| F13. Production retained-log search. [log-helpers.ts](../src/features/resource-detail/log-helpers.ts), [ResourceLogsPane.svelte](../src/features/resource-detail/ResourceLogsPane.svelte). `Log parse/append/search and retained-buffer filtering` | Extend. Helpers ready; extract inline retained filtering. | Already-parsed buffer, blank searches, reverse/overflow order, long metadata. Check existing retention bound, stable indices and search results; separate fill from eviction. | Retain. Per-stream-line append plus retained filtering at [ResourceLogsPane.svelte:107](../src/features/resource-detail/ResourceLogsPane.svelte), `:111`, `:180`. Filter input does not reparse retained lines. Replace the uncalled batch-reparse benchmark with this production path. |
| F14. Retained event search. [incident-events.ts](../src/features/resource-detail/incident-events.ts), [eventsTabModel.ts](../src/features/resource-detail/eventsTabModel.ts). `sortIncidentEvents; event summaries and filtering` | Extend. Ready; separate sort on data arrival from retained search. | Timestamp ties, reasons, counts and namespace collisions. Check stable order, scope and totals. | Retain. Event sorting at [ResourceDetailPanel.svelte:341](../src/features/resource-detail/ResourceDetailPanel.svelte) depends on event data. Summary/filter at [EventsTab.svelte:44](../src/features/resource-detail/EventsTab.svelte), `:45` have separate dependencies. Search repeats filtering, not sorting. |
| F15. Timeline construction and presentation. [incident-timeline.ts](../src/features/resource-detail/incident-timeline.ts), [incident-timeline-labels.ts](../src/features/resource-detail/incident-timeline-labels.ts), [ResourceTimeline.svelte](../src/features/resource-detail/ResourceTimeline.svelte). `Timeline builder, labels and day grouping` | Extend. Builders ready; extract grouping; fixed time zone/locale. | Duplicate, terminal, undated/invalid entries and logs spanning days. Check deduplication, order, labels and group boundaries. | Retain. Timeline assembly at [ResourceDetailPanel.svelte:357](../src/features/resource-detail/ResourceDetailPanel.svelte); grouping and row labels at [ResourceTimeline.svelte:40](../src/features/resource-detail/ResourceTimeline.svelte), `:107`, `:112`. Include raw-detail preparation and retained presentation separately. |
| F16. YAML diff branch coverage. [yamlTabDiff.ts](../src/features/resource-detail/yamlTabDiff.ts), [deployment-revisions-model.ts](../src/features/resource-detail/deployment-revisions-model.ts). `buildYamlDryRunDiff and shared comparison wrappers` | Extend. Ready. | Identical, insertion/deletion, repeated blocks, sparse edits, newline and style combinations. Check diff direction, context, headers and current truncation contract. | Retain. Preview diff at [ResourceYamlPane.svelte:134](../src/features/resource-detail/ResourceYamlPane.svelte); revision comparison at [DeploymentRevisionsTab.svelte:64](../src/features/resource-detail/DeploymentRevisionsTab.svelte). Diff construction precedes the 24-line preview slice at `ResourceYamlPane.svelte:145`. Preserve the existing diff suite and cover active branches. |
| F17. Diagnostic field-to-YAML ranges. [yamlTabDiff.ts](../src/features/resource-detail/yamlTabDiff.ts). `findYamlFieldRange` | New. Ready; batch diagnostic mapping. | Early/late/absent keys, quoted/regex-significant keys and repeated leaves. Check escaping, bounded offsets and current first-leaf/fallback behavior. | Fold. [ResourceYamlPane.svelte:359](../src/features/resource-detail/ResourceYamlPane.svelte) maps returned lint diagnostics through `findYamlFieldRange` at `:366`. This is the backend-result preparation leg of the lint pipeline, not a per-keystroke independent operation. |
| F18. Secret parsing and decoding. [secretData.ts](../src/features/resource-detail/secretData.ts). `parseSecretData; decodeSecretDataValue` | New. Ready; synthetic values only. | Masked/UTF-8/invalid/binary values and long YAML. Check current decoding/error semantics; reveal IPC stays outside. | Retain. [SecretDataViewer.svelte:12](../src/features/resource-detail/SecretDataViewer.svelte) parses a changed YAML document; `:37` decodes one chosen value on click. Benchmark these as distinct stages, not decoding every key on arrival. |
| F19. YAML document formatting. [yamlFormat.ts](../src/lib/yamlFormat.ts). `formatYamlDocument` | New. Ready. | Nesting, comments, quoted scalars, styles, duplicate keys and invalid input. Check value preservation, escaping, newline and error behavior. | Retain. Explicit formatting action at [ResourceYamlPane.svelte:382](../src/features/resource-detail/ResourceYamlPane.svelte); whole-document formatter at [yamlFormat.ts:17](../src/lib/yamlFormat.ts). Parsing/serialization cost scales with document bytes and nesting. It is not a typing-time formatter. |
| F20. Exec input chunking. [execInput.ts](../src/features/resource-detail/execInput.ts). `createExecInput` | No benchmark seam needed. | No benchmark fixture justified. Preserve necessary correctness coverage. | Remove. Xterm input calls the bounded queue, [ExecTab.svelte:187](../src/features/resource-detail/ExecTab.svelte); [execInput.ts:5](../src/features/resource-detail/execInput.ts) caps chunks at 16 KiB and pending input at 256 KiB. No observed paste workload or CPU regression question justifies a standalone controlled-async benchmark. Keep correctness coverage. |

### Editor and presentation

| Candidate and operation | Coverage and access | Fixture and correctness | Value decision and production evidence |
| --- | --- | --- | --- |
| F21. Local YAML diagnostics. [yamlCodeViewerDiagnostics.ts](../src/components/yamlCodeViewerDiagnostics.ts), [yamlFormat.ts](../src/lib/yamlFormat.ts). `yamlDiagnostics; parseYamlErrors` | New. Ready. | Valid/unfinished documents, duplicates, indentation, long flow values, blank and multiple errors. Check severity, bounded ranges and exceptions converted to diagnostics. | Retain. [YamlCodeEditor.svelte:103](../src/components/YamlCodeEditor.svelte) performs local diagnostics, optional returned-diagnostic mapping and lens dispatch. Linter delay is 450 ms at `:116`. Measure full-document lint work and application of prepared results, excluding IPC/waits. |
| F22. YAML editor decorations. [yamlCodeViewerExtensions.ts](../src/components/yamlCodeViewerExtensions.ts). `Decoration construction and retained edit updates` | New. Headless EditorState seam inspected, not execution-verified. | Deep indentation, quoted comments, URLs, blocks, long lines and fixed edits. Check classes/spans, RangeSet order and no-change retention. | Retain. [yamlCodeViewerExtensions.ts:26](../src/components/yamlCodeViewerExtensions.ts) builds decorations on creation and rebuilds on every document change. Scrolling/non-document transactions retain them. Whole-document scanning after a small edit is a concrete optimization question. |
| F23. Editor error lens. [YamlCodeEditor.svelte](../src/components/YamlCodeEditor.svelte). `Diagnostic grouping and mapped ranges` | New. Extract headless computation; exclude DOM widgets. | Multiple diagnostics per line, severities and edit mappings. Check stable grouping and valid mapped ranges. | Fold. [YamlCodeEditor.svelte:64](../src/components/YamlCodeEditor.svelte) groups diagnostics into lens decorations; `:87` maps retained ranges on transactions. Include prepared-diagnostic application and edit mapping, excluding DOM widget creation/rendering. |
| F24. Timestamp display batches. [timestamp-format.ts](../src/components/timestamp-format.ts), [TimestampText.svelte](../src/components/TimestampText.svelte). `Exact/time/relative formats used for title and display` | New. Ready; fixed time, time zone and locale. | Date precisions, UTC/local mode, invalid inputs and relative fallback. Check displayed values and fallback contracts. | Fold. Actual log/event row timestamp calls originate at [ResourceDetailPanel.svelte:526](../src/features/resource-detail/ResourceDetailPanel.svelte), `:530`, `:534`, `:538`. Batch only mounted result rows. Timeline locale/date work belongs to F15; it uses a different sequence. |
| F25. Error classification and redaction. [friendly-errors.ts](../src/lib/friendly-errors.ts), [error-redaction.ts](../src/lib/error-redaction.ts). `Friendly-error and redaction batch pipeline` | New. Ready; synthetic errors. | Long errors, typed precedence, permissions and credential markers. Check classification precedence and complete redaction. | Fold. Retain message redaction inside operation-progress/outcome shaping. Remove the artificial error-classification batch: [FriendlyError.svelte:32](../src/components/FriendlyError.svelte) classifies a displayed error/context change, not a fleet of errors on every input. |

### Sidebar and navigation

| Candidate and operation | Coverage and access | Fixture and correctness | Value decision and production evidence |
| --- | --- | --- | --- |
| F26. Shallow sidebar and expanded namespace updates. [workspaceShellModel.ts](../src/app/svelte/workspaceShellModel.ts), [sidebar-tree-helpers.ts](../src/components/sidebar-tree-helpers.ts). `buildSidebarTree and namespace children` | Extend. Ready. | Versions/groups/plurals/short names, scopes, providers and present-kind subsets. Check lazy child semantics, stable identities and ordering. | Retain. [SidebarTree.svelte:180](../src/app/svelte/SidebarTree.svelte) shapes the shallow tree on discovery/provider/settings/search changes. Namespace children are requested lazily at `:149`, through [SidebarTreeNode.svelte:43](../src/app/svelte/SidebarTreeNode.svelte). Existing suite needs accurate expansion fixtures. |
| F27. Expanded sidebar results. [SidebarTree.svelte](../src/app/svelte/SidebarTree.svelte), [SidebarTreeNode.svelte](../src/app/svelte/SidebarTreeNode.svelte). `Expanded namespaces and result/error/pending maps; membership` | New. Extract plain-data shaping; exclude query creation. | Namespace IDs with successful, failed and pending results. Check result-to-query pairing, disabled state and child IDs. | Fold. Expanded namespace selection and result/error/pending maps at [SidebarTree.svelte:90](../src/app/svelte/SidebarTree.svelte), `:114`, `:126`, `:139`. Include these as the expanded-sidebar update stage, with only expanded namespaces receiving queries/children. |
| F28. Available-kind merge. [workspaceShellModel.ts](../src/app/svelte/workspaceShellModel.ts). `appendPresentCustomResourceKinds; resourceBrowserAvailableKinds` | New. Ready. | Versioned custom kinds, fixed scopes and duplicate observations. Check full API identity and first-seen order. | Fold. Available-kind merge at [WorkspaceShell.svelte:305](../src/app/svelte/WorkspaceShell.svelte), implemented in [workspaceShellModel.ts:68](../src/app/svelte/workspaceShellModel.ts). It prepares browser scope options when discovered/present kinds or scope settings change. |
| F29. Navigation and path snapshots. [workspaceNavigation.ts](../src/app/svelte/workspaceNavigation.ts), [path-state.ts](../src/lib/path-state.ts), [WorkspaceShell.svelte](../src/app/svelte/WorkspaceShell.svelte). `navigateWorkspace equality; sanitize/encode/decode path state` | New. Exports ready; extract snapshot work if needed. | Resource/context/scope transitions and valid/invalid encoded state. Check preserved fields, IDs and defaults; no credentials; exclude browser history/storage. | Fold. Browser state emission at [ResourceBrowser.svelte:750](../src/features/resources/ResourceBrowser.svelte) reaches equality serialization at [workspaceNavigation.ts:281](../src/app/svelte/workspaceNavigation.ts) and snapshot persistence at [WorkspaceShell.svelte:408](../src/app/svelte/WorkspaceShell.svelte). Include pure snapshot comparison/sanitization/serialization as an interaction fixture; exclude storage/history. |
| F30. Pinned-entry normalization and membership. [workspace-entry-points.ts](../src/lib/workspace-entry-points.ts), [WorkspaceShell.svelte](../src/app/svelte/WorkspaceShell.svelte). `Pinned-entry dedupe and batch membership` | New. Exports ready; extract inline batch work. | Observed pin volumes, duplicate/full resource identities. Check stable order and exact identity; recent-entry bounds do not become pin limits. | Fold. Normalization and focused membership at [WorkspaceShell.svelte:319](../src/app/svelte/WorkspaceShell.svelte), `:325`; helpers at [workspace-entry-points.ts:84](../src/lib/workspace-entry-points.ts). Production builds a resource-key set for the table at [ResourceBrowser.svelte:688](../src/features/resources/ResourceBrowser.svelte). The proposed raw per-row membership batch is inaccurate. |

### Workspaces

| Candidate and operation | Coverage and access | Fixture and correctness | Value decision and production evidence |
| --- | --- | --- | --- |
| F31. Workspace health and comparisons. [workspace-model.ts](../src/lib/workspace-model.ts). `buildWorkspaceHealthSummary; buildWorkspaceCompareSummaries` | New. Ready. | Mixed contexts/namespaces, health, missing/overlapping rows and empty collections. Check scope partitions, counts/restarts and immutable inputs. | Retain. Workspace health at [WorkspaceOverview.svelte:186](../src/features/workspaces/WorkspaceOverview.svelte), comparisons at `:239`. Data changes trigger them. Production compares at most two selected partitions, not all context/namespace pairs. |
| F32. Authoritative entry reconciliation and changed-entry persistence. [workspace-entry-points.ts](../src/lib/workspace-entry-points.ts). `reconcileEntryPoints` | New. Ready; include live-key construction in measurement and prepare coverage. | Present/absent resources under authoritative and incomplete coverage. Check remove only proven covered absences, retain uncovered entries, dedupe/order; exclude persistence/clock. | Retain. Successful non-placeholder overview data invokes reconciliation at [WorkspaceOverview.svelte:181](../src/features/workspaces/WorkspaceOverview.svelte). [workspace-entry-points.ts:137](../src/lib/workspace-entry-points.ts) builds live keys inside the operation and checks coverage per resource entry. Do not prepare that index outside measurement. |
| F33. Fetch planning and restore status. [query.ts](../src/features/workspaces/query.ts), [workspace-model.ts](../src/lib/workspace-model.ts). `buildWorkspaceFetchKeys and plans; computeRestoreStatus` | New. Ready. | Cluster/all/explicit missing scopes, CRDs and contexts. Check primary discovery, identity and order. | Fold. Restore/scope planning at [WorkspaceOverview.svelte:134](../src/features/workspaces/WorkspaceOverview.svelte), `:142`, `:145`. Include workspace/multi-context planning as a caller-specific scope fixture, preserving missing-discovery behavior. |
| F34. Workspace sharing. [workspace-sharing-parse.ts](../src/features/workspaces/workspace-sharing-parse.ts), [workspace-sharing.ts](../src/features/workspaces/workspace-sharing.ts). `parseWorkspaceImport; export, preview and apply` | No benchmark seam needed. | No benchmark fixture justified. Preserve necessary correctness coverage. | Remove. Sharing runs on explicit import/export/apply actions, [WorkspaceLauncher.svelte:215](../src/features/workspaces/WorkspaceLauncher.svelte), `:233`, `:258`. No observed large shared collection or repeated interaction establishes value for a standalone import/export inventory. |
| F35. Persisted workspace validation. [workspaceStore.ts](../src/features/workspaces/workspaceStore.ts). `Read/sanitize/stringify boundaries` | New. Isolate in-memory JSON from storage. | Malformed nested fields and whole-collection envelopes. Check valid metadata preserved and invalid envelopes rejected. | Fold. Whole-collection serialization follows changed entry-point updates, [workspaceStore.ts:230](../src/features/workspaces/workspaceStore.ts), `:253`, and other store mutations at `:220`. Include changed-entry persistence preparation. Drop standalone cold validator benchmarking without a demonstrated workload. |

### Command palette

| Candidate and operation | Coverage and access | Fixture and correctness | Value decision and production evidence |
| --- | --- | --- | --- |
| F36. Palette cache harvesting and index construction. [entries.ts](../src/features/command-palette/entries.ts). `buildArgoSearchResources; dedupeResources; buildDedupedResourceSearchIndex` | Extend. Include production cache harvesting; prepare QueryClient/Argo state outside measurement. | All Argo resource types, overlapping cache sets and conflicting duplicates. Check full resource identity, first occurrence, ordering and ownership. | Retain. [CommandPalette.svelte:168](../src/app/svelte/CommandPalette.svelte) harvests matching caches and builds a deduplicated index when open/data/source/context dependencies change. Search input retains the index. Include cache harvesting, not only prebuilt arrays. |
| F37. Retained palette typing. [entries.ts](../src/features/command-palette/entries.ts), [CommandPalette.svelte](../src/app/svelte/CommandPalette.svelte). `filterNamespaces and production resource-filter sequence` | Extend. Exports ready; extract inline sequence. | Blank/whitespace/case queries, many/no hits and matching fields. Check post-filter cap and stable order; retain the search index. | Retain. Namespace/resource filtering at [CommandPalette.svelte:163](../src/app/svelte/CommandPalette.svelte), `:180` repeats on query input. Full filtering precedes caps of eight namespaces and 50 resources. Blank/whitespace queries bypass these result lists. |
| F38. Global search fetch planning. [entries.ts](../src/features/command-palette/entries.ts). `buildGlobalSearchFetchKeys` | New. Ready. | Custom versions, Flux kinds, cluster scopes and providers. Check kind deduplication and workspace Kubernetes versus provider-wide GitOps scope. | Fold. Global fetch planning at [CommandPalette.svelte:134](../src/app/svelte/CommandPalette.svelte), implemented by [entries.ts:47](../src/features/command-palette/entries.ts). Include provider-wide Flux plus workspace-scoped Kubernetes planning as a distinct fixture. |

### RBAC

| Candidate and operation | Coverage and access | Fixture and correctness | Value decision and production evidence |
| --- | --- | --- | --- |
| F39. Cockpit shaping and retained search. [cockpitModel.ts](../src/features/rbac/cockpitModel.ts). `cockpitItems and filters` | Extend. Ready; separate cold shaping and retained search. | Roles/bindings, service accounts/groups, missing refs, partial coverage and resource/non-resource rules. Check risk, unknowns, dedupe, ties and case/order semantics. | Retain. Cold cockpit at [RbacView.svelte:180](../src/features/rbac/RbacView.svelte); retained bucket/search at `:181`. Search reconstructs searchable rule/subject text. Existing suite conflates cold and retained work and covers only Service Accounts. |
| F40. Observed permission joins. [observedPermissions.ts](../src/features/rbac/observedPermissions.ts). `Permission merges and grants` | New. Ready. | Users/groups/service accounts, auto groups, roles/bindings and duplicate origins/fanout. Check scope, sorted unique origins, rule-array order and current duplicate-role behavior. | Retain. [RbacView.svelte:221](../src/features/rbac/RbacView.svelte) joins observed permissions on inspection or active identity changes. Manual identity name/namespace/groups inputs can repeat it; target verb/resource verifier fields do not drive this join. |
| F41. Review evidence fingerprints. [reviewModel.ts](../src/features/rbac/reviewModel.ts). `buildRbacEvidenceFingerprint; reviewStatus` | New. Ready. | Active/stale/absent reviews, reordered policies, inheritance and wrong identities. Check canonical hash semantics, relevant change detection, context matching and unreviewed fast path. | Retain. Visible badges resolve records and status, [RbacReviewBadge.svelte:21](../src/features/rbac/RbacReviewBadge.svelte), `:28`; selected panel at [RbacReviewPanel.svelte:40](../src/features/rbac/RbacReviewPanel.svelte); save hashes at `:52`. No-record status returns before hashing, [reviewModel.ts:44](../src/features/rbac/reviewModel.ts). |
| F42. Visible RBAC queue preparation. [RbacView.svelte](../src/features/rbac/RbacView.svelte). `queueTags; bindingSources; riskCount` | New. Extract visible-row batch computation. | Direct/group grants, risk categories, collisions and observed queue sizes. Check bucket counts over unsearched inventory, source identity and order. | Retain. Queue tags and sources at [RbacView.svelte:420](../src/features/rbac/RbacView.svelte), `:465`; bucket counts at `:416`. Visible Service Account tags rescan bindings. Queue initially mounts 50 rows, `:118`, and grows by 50 at `:756`. |

### Helm

| Candidate and operation | Coverage and access | Fixture and correctness | Value decision and production evidence |
| --- | --- | --- | --- |
| F43. Release grouping and reconciliation. [helpers.ts](../src/features/helm/helpers.ts). `Grouping/sorting and reconciliation helpers` | Extend. Ready. | Unavailable/missing/equal states, shuffled rows and namespace collisions. Check conservation and stable order; uncalled owned-resource helper is not active coverage. | Fold. Grouping is inside the active release-state helper. Reconciliation sorting at [HelmSurface.svelte:117](../src/features/helm/HelmSurface.svelte) is data-driven separately. Existing uncalled owned-resource helper is not production coverage. |
| F44. Complete Helm surface filtering. [surfaceState.ts](../src/features/helm/surfaceState.ts), [HelmView.svelte](../src/features/helm/HelmView.svelte). `buildHelmReleaseState; counts and retained search` | New. Exports ready; extract filter sequence if needed. | All search fields, blank/case queries, absent namespace and release states. Check unfiltered counts, grouping and retained ordering. | Retain. [HelmSurface.svelte:114](../src/features/helm/HelmSurface.svelte) calls `buildHelmReleaseState` on release/search/namespace changes. Grouping repeats with search. Counts at [HelmView.svelte:105](../src/features/helm/HelmView.svelte) depend only on release data. |

### GitOps

| Candidate and operation | Coverage and access | Fixture and correctness | Value decision and production evidence |
| --- | --- | --- | --- |
| F45. Inventory rails and assessed summaries. [surfaceModel.ts](../src/features/gitops/surfaceModel.ts), [surfaceSummary.ts](../src/features/gitops/surfaceSummary.ts), [surfaceState.ts](../src/features/gitops/surfaceState.ts). `Rails, summaries and provider tables` | Extend. Ready. | ApplicationSets/Projects, multiple Flux kinds, detected-empty/unavailable providers and assessed/partial health. Check provider totals, exact table identities and complete Flux summary independent of active kind. | Retain. Read-state, selections, tables and rails at [GitOpsSurface.svelte:144](../src/features/gitops/GitOpsSurface.svelte), `:156`, `:160`, `:161`; summary at [GitOpsView.svelte:129](../src/features/gitops/GitOpsView.svelte). Preserve existing suite, add assessed Argo health and actual row preparation. |
| F46. Source and revision presentation. [surfaceTooltips.ts](../src/features/gitops/surfaceTooltips.ts). `Source groups/lines and revision formatting` | New. Ready; measure the actual row sequence. | Git/Helm/plugin/unknown sources, duplicates, SHA and resolved targets. Check first-seen order, compaction and fallbacks. | Fold. Source/tooltip/revision preparation runs for every mounted card/list row, [GitOpsView.svelte:499](../src/features/gitops/GitOpsView.svelte), `:662`. The `@const` source work is eager before tooltip hover. Include the exact row sequence, not isolated tooltip helpers. |
| F47. Managed-resource workspace matching. [argo-workspace-model.ts](../src/features/gitops/argo-workspace-model.ts). `argoResourceCounts; filterWorkspaceResources; reconciliation filtering` | New. Ready. | Prune/health/sync states, API-group fallbacks, explicit core identities, malformed/duplicate/unmatched rows. Check exact identity/order, current plus needsSync totals and original-reference return when unchanged. | Retain. Browser counts/filter at [ResourceBrowser.svelte:615](../src/features/resources/ResourceBrowser.svelte), `:627`; inspector reconciliation/counts at [ArgoApplicationDetails.svelte:257](../src/features/gitops/ArgoApplicationDetails.svelte), `:339`. Managed identities and workspace rows form a real inventory join. |
| F48. Argo comparison preparation and highlighting. [argo-workspace-model.ts](../src/features/gitops/argo-workspace-model.ts), [ArgoApplicationDetails.svelte](../src/features/gitops/ArgoApplicationDetails.svelte). `Comparison lookup/document selection; serialization, modification and highlight preparation` | New. Exports ready; extract displayed-row/document work. | Resource/record joins, document bytes, diff complexity and rendered lines. Check authoritative flags/precedence, normalized states and text reconstruction; exclude collapsed work. | Retain. Displayed comparison rows at [ArgoApplicationDetails.svelte:912](../src/features/gitops/ArgoApplicationDetails.svelte); lookup at `:577`, serialization at `:642`, diff/highlighting at `:930`. Measure only active document branches, including repeated lookup/serialization actually performed. |
| F49. Application YAML fallback parsing. [ArgoApplicationDetails.svelte](../src/features/gitops/ArgoApplicationDetails.svelte). `applicationSpecFromYaml` | New. Extract local parser. | Large status, multiple sources and invalid roots/specs. Check exact spec and existing empty-object fallback. | Fold. Application YAML fallback is derived from changed YAML at [ArgoApplicationDetails.svelte:373](../src/features/gitops/ArgoApplicationDetails.svelte); whole-document parser at `:656`. Include as cold inspector preparation, not once per comparison row. |
| F50. Operation observation, progress and outcomes. [argo-operation-progress.ts](../src/features/gitops/argo-operation-progress.ts). `Progress and outcome builders` | New. Ready. | Large resource/hook arrays, malformed/sensitive messages, old terminal and fresh observations. Check redaction, old-result exclusion and completion semantics; exclude polling. | Retain. Observation calls progress and outcome consecutively, [argo-operation-tracker.ts:115](../src/features/gitops/argo-operation-tracker.ts); display also projects progress, [ArgoOperationProgress.svelte:5](../src/features/gitops/ArgoOperationProgress.svelte). Sync outcome itself builds progress. Resource/message arrays repeat on observations. |

### Incidents

| Candidate and operation | Coverage and access | Fixture and correctness | Value decision and production evidence |
| --- | --- | --- | --- |
| F51. Incident queue shaping. [model.ts](../src/features/incidents/model.ts). `buildIncidentSurfaceState` | New. Ready. | Health/severity/restarts, warnings, timestamp ties, identity collisions and disappearing selection. Check group/order and full counts, overlap semantics and input immutability. | Retain. [IncidentSurface.svelte:85](../src/features/incidents/IncidentSurface.svelte) calls the complete surface model on data, filter and selected-key changes. Selection also repeats counts/filter/sort/group. No queue page bound is applied by this model. |
| F52. Selected incident owner and guidance. [incident-actions.ts](../src/features/incidents/incident-actions.ts). `resolveIncidentOwner` | New. Ready. | Unrelated nodes, chains, cycles, branches and dynamic lookalikes. Check scoped identity, nearest actionable owner and warning/completeness semantics. | Retain. Selected-resource owner resolution at [IncidentSurface.svelte:232](../src/features/incidents/IncidentSurface.svelte); guidance at `:238`. Indexing/BFS consumes namespace topology, and detail enrichment scans conditions/containers before bounded output. |
| F53. Incident guidance assembly. [guidance.ts](../src/features/incidents/guidance.ts). `buildIncidentGuidance` | New. Ready. | Snapshot/detail merges, many conditions/containers, duplicates, restarts and loading/error/historical states. Check deduped bounded evidence, priority and suppression based on completeness. | Fold. Guidance is the selected-incident enrichment stage. [guidance.ts:109](../src/features/incidents/guidance.ts), `:124` scans and deduplicates evidence before slicing output to six. Benchmark topology arrival and detail arrival separately. |

### Diagnostics and live sessions

| Candidate and operation | Coverage and access | Fixture and correctness | Value decision and production evidence |
| --- | --- | --- | --- |
| F54. Diagnostics refresh and report preparation. [diagnostics.ts](../src/lib/diagnostics.ts), [diagnosticsSettingsModel.ts](../src/app/svelte/diagnosticsSettingsModel.ts), [diagnostics-report.ts](../src/lib/diagnostics-report.ts). `summarizeDurations; batch rows; createLatencyReport` | New. Fix state/clock; isolate current-date acquisition. | Existing bounded buffers, groups, aliases, missing samples and ties. Check counts, nearest-rank/rounding, order and privacy. | Retain. Mounted diagnostics refresh snapshots every 1,500 ms, [DiagnosticsSettings.svelte:64](../src/app/svelte/DiagnosticsSettings.svelte); summaries at [diagnostics.ts:159](../src/lib/diagnostics.ts); report on click at `DiagnosticsSettings.svelte:126`. Protect the CPU cost of diagnostics itself within actual buffer bounds. |
| F55. Usage process-tree flattening. [usage-metrics.ts](../src/lib/usage-metrics.ts). `flattenUsageMetricsBreakdown` | No benchmark seam needed. | No benchmark fixture justified. Preserve necessary correctness coverage. | Remove. Usage data polls every 2,000 ms when enabled, [AppUsageFooter.svelte:31](../src/app/svelte/AppUsageFooter.svelte); flattening at `:40` is consumed by the process-detail popover. Linear traversal of actual host processes has no representative captured scale or substantive optimization question here. |
| F56. Query memory profiling computation. [memory-profile.ts](../src/lib/memory-profile.ts). `collectQueryMemory` | No benchmark seam needed. | No benchmark fixture justified. Preserve necessary correctness coverage. | Remove. [memory-profile.ts:83](../src/lib/memory-profile.ts) installs an opt-in profiling hook; `:43` runs only when that hook is invoked. It serializes cached payloads to estimate JSON bytes, not heap usage. No normal product interaction calls it. |
| F57. Settings and connection profiles. [settings.ts](../src/lib/settings.ts), [argo-connection-policy.ts](../src/lib/argo-connection-policy.ts), [argo-application-inspection.ts](../src/lib/argo-application-inspection.ts). `mergePersistedSettings; profile normalization and policy projection` | No benchmark seam needed. | No benchmark fixture justified. Preserve necessary correctness coverage. | Remove. Settings merge is persistence hydration, [settings.ts:370](../src/lib/settings.ts); connection-policy projections respond to local profiles/status/settings. No source-backed growing workload or repeated expensive operation justifies grouping these small security-sensitive helpers into a benchmark family. |
| F58. Session refresh and saved-forward matching. [liveSessionReadModel.ts](../src/features/live-sessions/liveSessionReadModel.ts), [helpers.ts](../src/features/live-sessions/helpers.ts), [podExecHelpers.ts](../src/features/live-sessions/podExecHelpers.ts). `Session normalization, sorting and grouping` | New. Ready. | Mixed sources/contexts, legacy/canonical statuses and timestamps. Check collection conservation and mixed/per-type arrays; session I/O excluded. | Retain. Session queries poll at 2,500 ms, [LiveSessionsSurface.svelte:59](../src/features/live-sessions/LiveSessionsSurface.svelte); read model at `:76` filters, combines, sorts and splits. Query functions already sort each type. Include the actual repeated stages and saved-forward display matching. |
| F59. Saved-forward matching. [helpers.ts](../src/features/live-sessions/helpers.ts), [LiveSessionsView.svelte](../src/features/live-sessions/LiveSessionsView.svelte). `savedPortForwardMatchesSession and conflict checks` | New. Helpers ready; extract saved-by-live batch matching. | Observed saved/live volumes, auto/fixed ports, host conflicts and failed sessions. Check first match, full scope identity and current conflict semantics. | Fold. Every saved row searches visible sessions, [LiveSessionsView.svelte:303](../src/features/live-sessions/LiveSessionsView.svelte). Measure saved-by-live matching with early/late/missing matches. Conflict checks are start-action fixtures, not polling work. |
| F60. Service port-option parsing. [portForwardForms.ts](../src/features/live-sessions/portForwardForms.ts). `extractServicePortOptions` | No benchmark seam needed. | No benchmark fixture justified. Preserve necessary correctness coverage. | Remove. [PortForwardTab.svelte:82](../src/features/resource-detail/PortForwardTab.svelte) parses changed Service YAML only. [portForwardForms.ts:77](../src/features/live-sessions/portForwardForms.ts) splits the document, scans until block termination and sorts supported ports. Local/remote port typing does not reparse it; no large real port workload establishes standalone value. |

## Rust candidates

The value review retains 28 workload families, folds 15 helpers into named production workloads and rejects five standalone targets. "Retain" justifies an independent signal. "Fold" keeps the work in the named pipeline, including attributable variants where stated. "Remove" means no new benchmark is justified by the inspected workload. Original fixture/access notes are retained for audit traceability and do not override this decision. "Bridge" uses the existing `bench-support` feature; "Extract" separates local work from fetching. Production contracts and bounds remain authoritative.

### Resource pipelines

| Candidate and operation | Coverage and access | Fixture and correctness | Value decision and production evidence |
| --- | --- | --- | --- |
| R01. Batch typed summaries. [summary_core.rs](../src-tauri/crates/inspection/src/commands/resources/summary_core.rs), [summary_workloads.rs](../src-tauri/crates/inspection/src/commands/resources/summary_workloads.rs), [summary_cluster.rs](../src-tauri/crates/inspection/src/commands/resources/summary_cluster.rs). `core_resource_summaries; workload/cluster summary mapping` | New. Extract post-fetch mapping and control time. | Every supported kind; missing status; readiness, terminal workloads, multiple containers and conditions. Check per-kind status precedence, exact scope, missing-field behavior and timestamps. | Retain. `typed_resource_list_projection`. Resource-list loads map entire returned collections, [resources/summary.rs:33](../src-tauri/crates/inspection/src/commands/resources/summary.rs), `summary_core.rs:31`, `summary_workloads.rs:50`, `summary_cluster.rs:40`. Measure post-fetch projection, not LIST waiting. |
| R02. Metadata, ownership, restarts and legacy health. [metadata.rs](../src-tauri/crates/kube/src/commands/helpers/metadata.rs), [ownership.rs](../src-tauri/crates/kube/src/commands/helpers/metadata/ownership.rs), [health.rs](../src-tauri/crates/kube/src/commands/helpers/health.rs). `base_resource_summary; extract_git_ops_owner; pod_restarts; update_resource_health` | New. Bridge existing helpers; measure batches within the complete summary path. | Large annotations/labels/owner lists; competing Argo/Flux evidence; restart and readiness states. Check tracking precedence, display/controller-owner distinction, checked restart sums, unavailable health. | Fold. Into R01/R04 and production topology fixtures. [helpers/metadata.rs:54](../src-tauri/crates/kube/src/commands/helpers/metadata.rs) allocates identities and ownership summaries per row. Argo ownership is constructed twice through `extract_argo_app` and `extract_git_ops_owner`, `metadata.rs:83`, `metadata/ownership.rs:20`. Large unrelated annotations are not scanned wholesale. |
| R03. Evidence-based health. [health.rs](../src-tauri/crates/models/src/health.rs). `evaluate_health; condition_evidence; argo_health_assessment; argo_application_set_health_assessment` | New. Public functions ready. | Current/historical evidence, tied winners with distinct/repeated reasons, provider states and long messages. Check winning reasons, completeness, historical exclusion, Unknown versus NotEvaluated. | Fold. Into R01/R04/R29/R35 health-bearing fixtures. [health.rs:95](../src-tauri/crates/models/src/health.rs) makes evidence passes, collects winners, and deduplicates reasons with linear `contains` calls at `:170`. Actual callers determine evidence size. Do not manufacture large standalone evidence arrays. |
| R04. Dynamic resource list projection. [dynamic.rs](../src-tauri/crates/inspection/src/commands/resources/dynamic.rs), [topology_dynamic.rs](../src-tauri/crates/inspection/src/commands/resources/topology_dynamic.rs). `dynamic_resource_summary; dynamic_health_assessment; dynamic_topology_input` | New. Bridge helpers and control age. | Phase/Ready precedence, malformed conditions, unknown CRD semantics, Argo and cluster-scoped identities. Check exact dynamic identity, provider semantics, fallback state and health evidence. | Retain. `dynamic_resource_list_projection`. Every loaded DynamicObject is projected, [resources/dynamic.rs:235](../src-tauri/crates/inspection/src/commands/resources/dynamic.rs); identity copies and condition/evidence allocation occur at `:80` and `:132`. Fold dynamic topology mapping into R08/R10 fixtures. |
| R05. Scope planning and row deduplication. [scope.rs](../src-tauri/crates/inspection/src/commands/resources/scope.rs). `group_requests; fetch_namespaces; ownership_namespaces; dedupe_rows` | New. Bridge pure helpers. | Mixed typed/dynamic kinds; duplicate and overlapping requests; all-namespace scopes; same names in distinct identities. Check exactly-one-kind validation, all-namespace promotion, last-row replacement and scope isolation. | Retain. `resource_scope_local_assembly`. Scope requests group at [resources/scope.rs:138](../src-tauri/crates/inspection/src/commands/resources/scope.rs); returned batches are merged, enriched, and deduplicated at `:210`. Formatted identity keys and BTreeMap insertion scale with rows, `:110`. |
| R06. Flux ownership index build. [flux_inventory.rs](../src-tauri/crates/kube/src/commands/helpers/metadata/flux_inventory.rs). `parse_inventory_id; normalized_api_version; index construction in fetch_flux_ownership_index` | New. Bridge parser; extract construction from fetch. | Core/grouped identities, group underscores, version aliases, malformed/duplicate inventory targets and existing bounds. Check exact version identity, first-owner precedence, bounded work and explicit partialness. | Retain. `flux_ownership_index_build`. Scope cache reloads invoke [helpers/metadata/flux_inventory.rs:91](../src-tauri/crates/kube/src/commands/helpers/metadata/flux_inventory.rs); topology also reads the index. Every retained entry allocates parsed identity parts and owner/key strings, `:38`, `:149`. Measure construction after fetch. |
| R07. Flux scope filtering and resource enrichment. [flux_inventory.rs](../src-tauri/crates/kube/src/commands/helpers/metadata/flux_inventory.rs), [topology_collection.rs](../src-tauri/crates/inspection/src/commands/resources/topology_collection.rs). `filter_flux_ownership_index; enrich_resource_summaries_with_flux_inventory; temporary topology-summary preparation` | New. Bridge index fixtures; extract complete local preparation if needed. | Inventory size, requested namespace sets, hit/miss rows, explicit owners and partial indexes. Check cluster-scope inclusion, explicit-owner precedence, unmatched-row partialness and equivalent output. | Fold. Into R05 and R08. Namespace filtering scans requested namespaces for every index entry, [helpers/metadata/flux_inventory.rs:73](../src-tauri/crates/kube/src/commands/helpers/metadata/flux_inventory.rs), `:193`; enrichment allocates lookup keys per row, `:218`. Topology clones all summaries into a temporary vector, [resources/topology_collection.rs:280](../src-tauri/crates/inspection/src/commands/resources/topology_collection.rs). |

### Topology

| Candidate and operation | Coverage and access | Fixture and correctness | Value decision and production evidence |
| --- | --- | --- | --- |
| R08. Ownership graph and StatefulSet/PVC linking. [topology.rs](../src-tauri/crates/inspection/src/commands/resources/topology.rs). `build_resource_topology; add_statefulset_pvc_edges` | Extend. Existing bridge ready; add fixture branches. | StatefulSets, replicas and claims; CronJob chains; missing owners, duplicate identities, cluster scope, API versions, ports/revisions and noise. Check uID ownership, namespace-qualified PVC links, unique ordered nodes/edges and warnings. | Retain. `ownership_topology`. [resources/topology.rs:342](../src-tauri/crates/inspection/src/commands/resources/topology.rs) invokes the graph builder on cache loads. It repeatedly constructs full nodes to derive IDs, `:155`, `:163`, `:189`; PVC linking scans StatefulSet Pods per claim and formats suffixes inside the scan, `:290`. |
| R09. Network-flow graph construction. [topology_network.rs](../src-tauri/crates/inspection/src/commands/resources/topology_network.rs). `build_network_flow_topology` | New. Pure inputs/builder exist; add bridge. | Ingresses, Services, EndpointSlices and Pods; selectors, fallback, repeated/missing targets, namespace collisions and ExternalName. Check slice precedence, relation types, non-selectable slices, edge deduplication and warning rules. | Retain. `network_flow_topology`. Production selects this builder at [resources/topology.rs:348](../src-tauri/crates/inspection/src/commands/resources/topology.rs). Each Service scans EndpointSlices, then scans Pods when slices are absent, `topology_network.rs:102`, `:151`. Node copies, edge identity allocation, and final sorting add bulk work. |
| R10. Production topology input mapping. [topology.rs](../src-tauri/crates/inspection/src/commands/resources/topology.rs), [topology_collection.rs](../src-tauri/crates/inspection/src/commands/resources/topology_collection.rs). `input_from_metadata; inputs_from_metadata; inputs_from_partial_metadata; pod_input; collection mappings` | New. Bridge local helpers; extract inline mapping and control time. | Typed/partial/dynamic objects; container ports, revision labels, selectors, ingress paths and endpoint references. Check metadata-only Secret/ConfigMap reads, controller-owner selection, unique sorted port hints. | Fold. Into complete R08/R09 input-to-graph fixtures. Production metadata mapping clones labels/owners, [resources/topology.rs:102](../src-tauri/crates/inspection/src/commands/resources/topology.rs); Pod mapping formats, sorts, and deduplicates port hints, `topology_collection.rs:522`. Existing synthetic bridge conversion does not exercise this mapping. |

### Caches and infrastructure

| Candidate and operation | Coverage and access | Fixture and correctness | Value decision and production evidence |
| --- | --- | --- | --- |
| R11. Ready cache hits and namespace coverage reads. [live_store.rs](../src-tauri/crates/kube/src/commands/live_store.rs). `SharedCache::peek; get_or_load; resources_for_scope` | New. Public ClusterLiveStore methods ready; prepare cache state. | Summary strings/evidence, topology/Flux index payloads; all-namespace entries filtered to named scopes. Check exact rows, freshness, source/context isolation and no loader calls on hits. | Retain. `cache_payload_reuse_and_replacement`. Ready hits clone complete values, [live_store.rs:97](../src-tauri/crates/kube/src/commands/live_store.rs), `:123`. Named-scope reuse first clones the entire all-namespace collection, then filters, `:493`. This runs on resource/scope/topology reads, including ready hits. |
| R12. Watch-driven invalidation. [live_store.rs](../src-tauri/crates/kube/src/commands/live_store.rs), [watch.rs](../src-tauri/crates/live/src/commands/streams/watch.rs). `mark_watch_resource_dirty; mark_dirty_where` | New. Public method ready; bridge mixed cache states. | Mixed contexts/kinds/namespaces, ready/loading entries, typed/dynamic aliases and topology scopes. Check exact/all-namespace invalidation, dirty loading state and unrelated-context retention. | Fold. Into R11's watch-dirty replacement fixture. Each non-bookmark resource event invalidates caches, [streams/watch.rs:227](../src-tauri/crates/live/src/commands/streams/watch.rs). Topology/present-kind invalidation scans keys and repeatedly formats the same prefix, [live_store.rs:555](../src-tauri/crates/kube/src/commands/live_store.rs). Measure its contribution within payload replacement. |
| R13. Explicit refresh eviction. [live_store_refresh.rs](../src-tauri/crates/kube/src/commands/live_store_refresh.rs). `refresh_view; evict_where` | No benchmark seam needed. | No benchmark fixture justified. Preserve necessary correctness coverage. | Remove. Explicit refresh performs key matching, eviction, and cancellation, [live_store_refresh.rs:27](../src-tauri/crates/kube/src/commands/live_store_refresh.rs). Dimensions are cache entries, requested kinds, and namespace overlap. No supplied workload establishes independent consequential CPU work beyond control bookkeeping and payload destruction. |
| R14. Load/reload bookkeeping and budget trimming. [live_store.rs](../src-tauri/crates/kube/src/commands/live_store.rs). `get_or_load; trim_to_budget; cancel_loading` | New. Bridge deterministic states/loaders; keep scheduling separate. | Immediate loads, failed reloads, prior dirty values and entries around the existing budget. Check single-load joins, generation protection, restored errors and existing eviction order. | Fold. Into R11's local miss/reload fixture. Successful completion clones the loaded payload into storage, [live_store.rs:178](../src-tauri/crates/kube/src/commands/live_store.rs). Keep that copy/drop cost; do not create separate measurements for load IDs, cancellation, or bounded trimming at `:253`. |
| R15. Cache keys and diagnostic state scans. [live_store.rs](../src-tauri/crates/kube/src/commands/live_store.rs), [live_store_diagnostics.rs](../src-tauri/crates/kube/src/commands/live_store_diagnostics.rs). `topology_cache_key; present_custom_resource_kinds_cache_key; diagnostics` | Extend. Existing diagnostic/scope-key bridges; topology-key bridge needed. | Duplicate/reordered/empty namespaces; every cache family; many ready/loading/dirty/previous entries. Check equivalent namespace-set keys, context/mode isolation, counters, redaction and shallow-weight semantics. | Fold. Namespace-key normalization belongs in R11's real cache-read fixtures, [live_store.rs:640](../src-tauri/crates/kube/src/commands/live_store.rs), `:658`. Remove standalone diagnostic-weight timing: `live_store_diagnostics.rs:81` scans entries and reads collection lengths, not individual resources or strings. |
| R16. Client cache and cancellation registry bookkeeping. [client_cache.rs](../src-tauri/crates/kube/src/commands/helpers/client_cache.rs), [cancellation.rs](../src-tauri/crates/kube/src/commands/cancellation.rs). `Cache::lookup/store; rotate_client_generation; register/cancel_scope/unregister` | No benchmark seam needed. | No benchmark fixture justified. Preserve necessary correctness coverage. | Remove. Client lookup/store and request cancellation manage small keys, handles, and tokens, [helpers/client_cache.rs:34](../src-tauri/crates/kube/src/commands/helpers/client_cache.rs), [cancellation.rs:49](../src-tauri/crates/kube/src/commands/cancellation.rs). The client cache has its existing 32-entry cap. TLS/client construction and file fingerprinting are separate work. No justified bulk bookkeeping workload was supplied. |
| R17. CRD catalog projection and sorting. [discovery.rs](../src-tauri/crates/inspection/src/commands/discovery.rs), [namespaces.rs](../src-tauri/crates/inspection/src/commands/namespaces.rs). `discovered_kind_from_crd; preferred_crd_version; sort_and_dedup_kinds; namespaces_summary_from` | Extend. Bridge CRD helpers; extract Namespace mapping and control time. | Served/storage versions, short names, scopes, duplicates and absent timestamps. Check served-version preference, exact catalog identity, names, ages and created_at. | Retain. `crd_catalog_projection_and_sort`, maintaining existing catalog-sort coverage. CRDs are projected and sorted on catalog loads, [discovery.rs:37](../src-tauri/crates/inspection/src/commands/discovery.rs), `:46`. Fold Namespace projection, [namespaces.rs:42](../src-tauri/crates/inspection/src/commands/namespaces.rs), into R01. |
| R18. Kubeconfig source parsing, merge and context projection. [kubeconfig.rs](../src-tauri/crates/kube/src/commands/kubeconfig.rs). `parse_path_input; normalize_paths; append_unique_paths; local config merge/context projection` | New. Bridge source-text parsing, merge and context projection; exclude disk/environment access. | Synthetic config only; duplicate paths/names, merge precedence, native path-list forms and current context. Check stable path order, platform semantics, no credential disclosure and fail-closed validation. | Retain. `kubeconfig_source_merge_context_projection`. Configured sources are parsed and merged at [kubeconfig.rs:307](../src-tauri/crates/kube/src/commands/kubeconfig.rs); fail-closed validation can repeat parsing before client-cache lookup, `kubeconfig_clients.rs:34`, `kubeconfig.rs:144`. Drop independent path/reorder/append helper cases. |
| R19. Diagnostics snapshots and error classification. [diagnostics.rs](../src-tauri/crates/kube/src/commands/diagnostics.rs), [error.rs](../src-tauri/crates/models/src/error.rs). `record_backend_timing; stored_backend_diagnostics; kube_error_kind; status_kind` | No benchmark seam needed. | No benchmark fixture justified. Preserve necessary correctness coverage. | Remove. Timing recording and error classification allocate small diagnostic records and scan short error structures. Snapshot history is bounded, [diagnostics.rs:14](../src-tauri/crates/kube/src/commands/diagnostics.rs), `:125`; debug settings poll it, [DiagnosticsSettings.svelte:41](../src/app/svelte/DiagnosticsSettings.svelte). Missing coverage alone does not justify another measurement family. |

### YAML and details

| Candidate and operation | Coverage and access | Fixture and correctness | Value decision and production evidence |
| --- | --- | --- | --- |
| R20. Complete typed/dynamic YAML and Secret pipeline. [serialization.rs](../src-tauri/crates/kube/src/commands/helpers/serialization.rs), [dynamic.rs](../src-tauri/crates/inspection/src/commands/resources/dynamic.rs), [yaml.rs](../src-tauri/crates/inspection/src/commands/resources/yaml.rs), [core.rs](../src-tauri/crates/inspection/src/commands/resources/details/core.rs). `serialize_resource_document; normalize_k8s_yaml_value; redact_secret; serialize_dynamic_resource_document` | Extend. Bridge helpers; extract full Secret preparation when isolating it. | Large managedFields/status/CRD schema/ConfigMap strings; Secret data/annotations; both view modes and encodings. Check parse-back equivalence, ApplyClean removal, preserved user fields and complete redaction. | Retain. `resource_document_prepare_emit`. Production converts entire objects, normalizes/redacts, and emits YAML/KYAML, [helpers/serialization.rs:54](../src-tauri/crates/kube/src/commands/helpers/serialization.rs). Typed Secret YAML first emits a document that is discarded, then redacts and emits again, [resources/yaml.rs:131](../src-tauri/crates/inspection/src/commands/resources/yaml.rs). |
| R21. Selected-resource detail assembly. [core.rs](../src-tauri/crates/inspection/src/commands/resources/details/core.rs), [workloads.rs](../src-tauri/crates/inspection/src/commands/resources/details/workloads.rs), [cluster.rs](../src-tauri/crates/inspection/src/commands/resources/details/cluster.rs), [dynamic.rs](../src-tauri/crates/inspection/src/commands/resources/dynamic.rs), [gitops_crd.rs](../src-tauri/crates/gitops/src/commands/gitops_crd.rs). `post-fetch detail builders; resource_yaml/metadata/status` | Extend. Extract post-fetch assembly; shared encoder already covered. | Large typed/dynamic metadata, status, nested specs and schemas. Check complete DTO fields, missing status, exact scope, encoding/view semantics and Secret redaction. | Fold. Into R20's `resource_inspection_details` fixtures. Detail builders additionally materialize metadata/status and summaries, [resources/details/core.rs:20](../src-tauri/crates/inspection/src/commands/resources/details/core.rs), `details/cluster.rs:186`, `resources/dynamic.rs:351`. Existing formatter cases omit those allocations. |
| R22. Deployment revision history shaping. [revisions.rs](../src-tauri/crates/inspection/src/commands/resources/revisions.rs). `deployment_revisions_from_replica_sets; is_owned_by_deployment; deployment_selector` | New. Bridge pure functions. | Large templates; foreign/non-controller owners; missing/invalid/tied revisions. Check exact UID/kind/API ownership, descending revision order and name tie-breaks. | Retain. `deployment_revision_history`. Every accepted ReplicaSet serializes its complete Pod template before revision sorting, [resources/revisions.rs:69](../src-tauri/crates/inspection/src/commands/resources/revisions.rs), `:78`, `:93`. Trigger is revision-tab activation or Deployment selection, not comparison selection over already-loaded revisions. |
| R23. Editor parsing, target validation and local lint. [validation.rs](../src-tauri/crates/inspection/src/commands/resources/apply/validation.rs), [linting.rs](../src-tauri/crates/inspection/src/commands/resources/apply/linting.rs). `parse_single_document; validate_yaml_apply; collect_local_lint_diagnostics` | New. Bridge existing pure boundaries. | YAML/KYAML drafts, empty/comments/null, multiple documents, malformed input, scope/identity mismatches and Secret targets. Check one-document contract, exact target binding, Secret rejection and precise diagnostic paths. | Retain. `editor_local_lint` and `editor_apply_validation`. Lint/preview/apply invoke parsing and validation, [resources/apply.rs:16](../src-tauri/crates/inspection/src/commands/resources/apply.rs), `:26`, `:38`. YAML parsing and YAML-to-JSON conversion, `apply/validation.rs:110`, dominate small identity checks. |
| R24. Kubeconform result decoding. [kubeconform.rs](../src-tauri/crates/inspection/src/commands/resources/kubeconform.rs). `collect_kubeconform_output; json_pointer_to_field_path; concise_validation_message` | New. Bridge captured-output processing. | Valid/invalid/skipped/error/unknown results; unreadable output, escaped pointers and missing errors. Check incomplete-validation notes, status classification, first-error behavior and field paths. | Fold. Into R23's `editor_schema_output_projection` fixture. Completed sidecar output is decoded at [resources/kubeconform.rs:84](../src-tauri/crates/inspection/src/commands/resources/kubeconform.rs); all validation errors are decoded although projection selects the first, `:138`. Keep subprocess/schema-download waiting outside measurement. |

### Helm

| Candidate and operation | Coverage and access | Fixture and correctness | Value decision and production evidence |
| --- | --- | --- | --- |
| R25. Storage decode and full release-record construction. [storage.rs](../src-tauri/crates/gitops/src/commands/helm/storage.rs), [values.rs](../src-tauri/crates/gitops/src/commands/helm/values.rs), [redaction.rs](../src-tauri/crates/gitops/src/commands/helm/redaction.rs). `decode_helm_release; secret_record; configmap_record; values_summary` | New. Bridge records; control timestamps; encode fixtures before timing. | Compressed/legacy data, both storage kinds, many retained revisions, chart/config/manifest payloads and malformed/bounded inputs. Check distinct errors, decoded identity precedence, metadata/value privacy and retained decode errors. | Retain. `helm_storage_records_to_latest_releases`. Every listed storage record is decoded and fully constructed before latest selection, [helm/storage.rs:323](../src-tauri/crates/gitops/src/commands/helm/storage.rs), `:356`, `:148`. Base64, gzip, JSON, values, manifest parsing, and discarded payload destruction are real bulk work. |
| R26. Latest-release selection. [storage.rs](../src-tauri/crates/gitops/src/commands/helm/storage.rs). `latest_releases` | New. Bridge private record type/helper. | Long histories, shuffled records, namespace/name collisions and absent/equal revisions. Check greater-revision replacement, first-entry ties, namespace ordering; state whether discarded-record drops are timed. | Fold. Into R25. Latest selection consumes full records, clones namespace/name keys, and drops discarded record payloads, [helm/storage.rs:613](../src-tauri/crates/gitops/src/commands/helm/storage.rs). Lightweight summary-only fixtures would miss production destruction cost. |
| R27. Helm manifest parsing. [manifest.rs](../src-tauri/crates/gitops/src/commands/helm/manifest.rs). `manifest_resources; manifest_summary_from_resources` | Extend. Existing bridge ready. | Mixed/custom kinds, large embedded config, missing identities/namespaces, malformed/empty documents and preview boundaries. Check document order, parser skip semantics, preview count and truncation flag. | Retain. Maintain `helm_manifest_parse`; expand its documents. [helm/manifest.rs:34](../src-tauri/crates/gitops/src/commands/helm/manifest.rs) materializes whole YAML documents to extract a few identity fields. Fold the bounded preview copier into R25 rather than keeping it as an independent bulk measurement. |
| R28. Reconciliation preparation and merge. [reconciliation.rs](../src-tauri/crates/gitops/src/commands/helm/reconciliation.rs). `discovered_ref_index; resource_identity_from_manifest; conservative_label_scan_kinds; reconciliation_totals; merge/sort` | New. Bridge helpers; extract result merge from GET/LIST orchestration. | Discovery/manifests, unknown scope/kinds, duplicate refs, label-only rows and every result status. Check namespaced fallback only, manifest precedence, conservative scans, unavailable outcomes and ordering. | Retain. `helm_reconciliation_prepare` and `helm_reconciliation_assemble`. Discovery/ref indexes, joins, merge/sort, and totals surround GET/LIST waits, [helm/reconciliation.rs:125](../src-tauri/crates/gitops/src/commands/helm/reconciliation.rs), `:162`, `:78`. Sort-key construction clones tuple strings, `:429`. |

### Argo and Flux

| Candidate and operation | Coverage and access | Fixture and correctness | Value decision and production evidence |
| --- | --- | --- | --- |
| R29. Argo list summary projection. [applications.rs](../src-tauri/crates/gitops/src/commands/argo/applications.rs), [appsets.rs](../src-tauri/crates/gitops/src/commands/argo/appsets.rs), [appprojects.rs](../src-tauri/crates/gitops/src/commands/argo/appprojects.rs). `application_summary_from_object; application_set_summary_from_object; app_project_summary_from_object` | New. Bridge helpers and control ages. | Resource/source/revision arrays, namespace sets, conditions, generators and pending operations. Check source/revision alignment, HEAD fallback, sorted namespaces, pending-operation precedence and provider health. | Retain. `argo_list_projection`. Production projects complete fetched batches, [argo/applications.rs:109](../src-tauri/crates/gitops/src/commands/argo/applications.rs), `appsets.rs:101`, `appprojects.rs:101`. Application source/revision copies and namespace-set construction scale with actual arrays. AppSet generator-array length is not a bulk dimension because only the first generator parameter is read. |
| R30. Kubernetes and Connected inspector assembly. [connected.rs](../src-tauri/crates/gitops/src/commands/argo/connected.rs), [transport.rs](../src-tauri/crates/gitops/src/commands/argo/transport.rs). `inspector_from_application; connected_inspector; managed_resource; history; state; redact_secret_fields` | New. Bridge helpers; reset mutable redaction inputs outside measurement. | Resources/history/conditions trees; object/JSON/YAML/malformed/absent states, response envelopes and nested Secrets. Check provenance/exactness, optional identities, state order, modified flags and no secret sentinels. | Retain. `argo_kubernetes_inspector` and `argo_connected_inspector`. Inspector reads repeatedly clone/project status, resources, history, conditions, and state trees, [argo/connected.rs:829](../src-tauri/crates/gitops/src/commands/argo/connected.rs), `:924`, `:1108`. State parsing/redaction occurs at `:777`, [argo/transport.rs:124](../src-tauri/crates/gitops/src/commands/argo/transport.rs). |
| R31. Selected-resource comparison lookup and shaping. [comparison.rs](../src-tauri/crates/gitops/src/commands/argo/comparison.rs). `argo_resource_comparison local lookup/projection` | New. Extract local stage from HTTP orchestration. | First/late/missing target matches in large responses and redacted Secret states. Check exact group/version/kind/namespace/name, NotFound behavior and redaction. | Fold. Into R30's `argo_selected_comparison` fixtures. Kubernetes comparison builds an inspector before lookup, [argo/comparison.rs:26](../src-tauri/crates/gitops/src/commands/argo/comparison.rs); connected comparison redacts the whole response, scans identities, then parses selected states, `:79`, `:86`, `:106`. Preserve these distinct paths. |
| R32. Argo Kubernetes identity revalidation. [operations.rs](../src-tauri/crates/gitops/src/commands/argo/operations.rs). `connected_payload; kubernetes_sync_payload; recorded_retry; sync_payload; verify_kubernetes_application_identity` | New. Bridge identity validation only; exclude execution and session storage. | Complete prepared Applications and matching/stale reviewed identities. Check every rejection and exact identity binding. | Retain. `argo_kubernetes_identity_revalidation`. Reviewed execution checks at [argo/operations.rs:388](../src-tauri/crates/gitops/src/commands/argo/operations.rs) and `:643`; the helper serializes the entire Application before checking a few fields, `:402`. Drop standalone options/payload bookkeeping and hypothetical large selected-resource arrays. |
| R33. Connection/profile capability projection. [connected.rs](../src-tauri/crates/gitops/src/commands/argo/connected.rs), [argo.rs](../src-tauri/crates/models/src/argo.rs). `servicetunnel_capabilities; normalize_service_tunnel_endpoint; ArgoConnectionProfile::deserialize` | No benchmark seam needed. | No benchmark fixture justified. Preserve necessary correctness coverage. | Remove. Connection capabilities/profile normalization perform Service-name filtering, port scans, URL/DNS validation, and small DTO construction, [argo/connected.rs:501](../src-tauri/crates/gitops/src/commands/argo/connected.rs), [argo.rs:316](../src-tauri/crates/models/src/argo.rs). Source evidence does not establish an independent bulk optimization question. |
| R34. Flux inventory and bounded status projection. [mod.rs](../src-tauri/crates/gitops/src/commands/flux/mod.rs). `inventory; inventory_partial; bounded_status` | New. Bridge local helpers. | Absent/malformed inventory, blank IDs, version aliases and inputs around/beyond existing bounds. Check prefix-before-filter behavior, order, trimming, partialness and unrelated status preservation. | Fold. Inventory projection belongs in R35, [flux/mod.rs:378](../src-tauri/crates/gitops/src/commands/flux/mod.rs). `inventory_partial` is a length check. `bounded_status` clones before truncating, `:298`, but its details command has no current frontend call beyond its wrapper. Do not claim a repeated UI workload for it. |
| R35. Flux summary and installed-kind matching. [mod.rs](../src-tauri/crates/gitops/src/commands/flux/mod.rs), [gitops_crd.rs](../src-tauri/crates/gitops/src/commands/gitops_crd.rs). `flux_summary; ready_status/message; installed_flux_kinds_from_resources; find_discovered_api_resource` | New. Bridge helpers; control age. | Early/late Ready conditions, suspension, sources/revisions, inventories and unrelated/alternate discovered APIs. Check first-Ready semantics, fallback fields, catalog order and exact discovered identity. | Retain. `flux_list_projection`. Each loaded object is summarized, [flux/mod.rs:163](../src-tauri/crates/gitops/src/commands/flux/mod.rs); repeated Ready scans, inventory copies, and health allocation occur at `:273`, `:378`, `:425`. Include installed-kind matching once per list preparation, not once per object. |

### RBAC, metrics and incidents

| Candidate and operation | Coverage and access | Fixture and correctness | Value decision and production evidence |
| --- | --- | --- | --- |
| R36. RBAC role-risk inheritance and namespace rollups. [rbac.rs](../src-tauri/crates/inspection/src/commands/rbac.rs), [rbac_risk.rs](../src-tauri/crates/inspection/src/commands/rbac_risk.rs). `inherit_binding_risks; namespace_access_summary; dedupe_subjects; dedupe_risks` | New. Bridge pure helpers. | Roles/bindings/subjects/namespaces, early/late/missing refs, repeated names, cluster-wide users/groups and partial coverage. Check namespace-qualified roles/SAs, coverage-sensitive unknown risks, binding counts, deduplication and output order. | Retain. `rbac_inventory_projection_and_rollup`. Production inheritance searches role arrays per binding, [rbac.rs:230](../src-tauri/crates/inspection/src/commands/rbac.rs); namespace rollup copies cluster-wide subjects and risks into buckets, `:409`. This is a genuine join/allocation workload. |
| R37. RBAC rule/role projection. [rbac.rs](../src-tauri/crates/inspection/src/commands/rbac.rs), [rbac_risk.rs](../src-tauri/crates/inspection/src/commands/rbac_risk.rs). `rule_summary; normalize_values; role_summary; rule_risks; binding_risks` | New. Bridge helpers; control role age. | Read/wildcard/Secret/RBAC-write/escalation/interactive rules and duplicates. Check sorted unique values, existing whitespace semantics and exact risk evidence. | Fold. Into R36's raw-inventory fixture. Role/rule projection constructs normalized sets and additional risk sets, then copies/deduplicates risks, [rbac.rs:311](../src-tauri/crates/inspection/src/commands/rbac.rs), `:383`, [rbac_risk.rs:4](../src-tauri/crates/inspection/src/commands/rbac_risk.rs). Standalone scalar/rule cases fragment the inventory operation. |
| R38. Metrics quantity and container normalization. [metrics.rs](../src-tauri/crates/inspection/src/commands/metrics.rs). `parse_cpu_millicores; parse_memory_bytes; container_usage_totals; metric_from_object` | New. Bridge pure helpers. | Pod/Node metrics, many containers, binary/decimal quantities and invalid/missing/overflow samples. Check finite CPU sums, checked/sticky memory overflow, availability, identity and timestamps. | Fold. Into R39's raw-metrics fixture. Every metric object is normalized before rollup, [metrics.rs:485](../src-tauri/crates/inspection/src/commands/metrics.rs). Quantity strings are copied before parsing, `:128`; measure complete container normalization rather than individual numeric parsers. |
| R39. Resource metrics normalization and rollup. [metrics.rs](../src-tauri/crates/inspection/src/commands/metrics.rs). `aggregate_workload_metrics; add_workload_metric; controller_owner` | New. Bridge pure helpers. | Pod/ReplicaSet/Deployment chains, direct owners, missing samples, namespace collisions and overflow. Check owner precedence, parent contribution, greatest timestamp and source-Pod ordering. | Retain. `resource_metrics_projection_and_rollup`. Builds metric/owner indexes, clones tuple keys, and contributes Pods to direct and parent workloads, [metrics.rs:347](../src-tauri/crates/inspection/src/commands/metrics.rs), `:305`, `:518`. |
| R40. Incident joins, evidence and sorting. [incidents.rs](../src-tauri/crates/inspection/src/commands/incidents.rs), [health.rs](../src-tauri/crates/inspection/src/commands/incidents/health.rs), [restarts.rs](../src-tauri/crates/inspection/src/commands/incidents/restarts.rs). `build_incident_items; incident_item_sort; event_match_sort; restart_evidence_from_pod` | New. Bridge private inputs; extract warning conversion and control time. | Healthy/degraded/restarted/warning-only objects; current/historical signals, tied/missing timestamps and recovered/active failures. Malformed typed Event timestamps are rejected before the post-fetch pipeline. Check affected-resource inclusion, historical evidence rules, existing event ceilings, namespace-qualified restart classifications and order. | Retain. `incident_cockpit_post_fetch_assembly`. Warning sorting repeatedly parses timestamps; restart extraction builds maps/vectors; incident assembly builds JSON evidence and sorts again, [incidents.rs:202](../src-tauri/crates/inspection/src/commands/incidents.rs), `:322`, `:459`, `incidents/restarts.rs:60`. |
| R41. Selected-resource Events pipeline. [events.rs](../src-tauri/crates/inspection/src/commands/events.rs). `event_matches_resource; summarize_event; local resource_events_from pipeline` | New. Bridge helpers; extract post-fetch pipeline and control time. | UID replacements/missing UID, shuffled/tied timestamps and source fallbacks. Check uID binding, existing missing-UID behavior, timestamp/source precedence and sort/reverse semantics. | Retain. `selected_resource_events_post_fetch`. After server-side filtering, Rust filters UID, sorts/reverses, and summarizes Events, [events.rs:108](../src-tauri/crates/inspection/src/commands/events.rs). Event-watch invalidation repeats the read, `ResourceDetailPanel.svelte:469`. Do not model this as a cluster-wide Event scan. |

### Streams, sessions and usage

| Candidate and operation | Coverage and access | Fixture and correctness | Value decision and production evidence |
| --- | --- | --- | --- |
| R42. Bounded log byte decoding. [logs.rs](../src-tauri/crates/live/src/commands/streams/logs.rs). `read_log_line` | New. Bridge helper; prepare reader/runtime outside measurement. | Short/buffer-spanning/oversized lines, LF/CRLF, EOF, invalid UTF-8 and multibyte boundaries. Check existing retention bound, draining to next line, truncation notices and errors. | Retain. `bounded_log_line_decode_and_drain`. Every Pod/aggregate log line calls the decoder, [streams/logs.rs:56](../src-tauri/crates/live/src/commands/streams/logs.rs), `aggregate_logs.rs:398`. Work includes newline scans, buffer growth, UTF-8 validation, and draining bytes beyond the retention limit, `logs.rs:103`. |
| R43. Exec incremental UTF-8 decoding. [runner.rs](../src-tauri/crates/live/src/commands/pod_exec/runner.rs). `take_utf8_prefix` | New. Bridge pure helper. | ASCII, split multibyte chunks, repeated invalid patterns and incomplete EOF tails. Check exact consumption/replacement, retained incomplete bytes and concatenated output. | Retain. `exec_incremental_utf8_decode`. Every stdout/stderr read calls the prefix loop, [pod_exec/runner.rs:56](../src-tauri/crates/live/src/commands/pod_exec/runner.rs). Each invalid prefix can allocate an output string and drain/move the remaining bytes, `:40`. |
| R44. Aggregate-log source enumeration. [aggregate_logs.rs](../src-tauri/crates/live/src/commands/streams/aggregate_logs.rs). `log_sources_from_pods; log_sources_from_pod; reconcile_sources local planning` | New. Bridge pure source enumeration; exclude task reconciliation, abort and spawn. | Pods/containers, deleting/malformed objects and totals beyond existing active-source cap. Check regular-container selection, trimming, order, pre-truncation total and equivalent source changes. | Retain. `aggregate_log_source_enumeration`. Pod watch changes trigger re-enumeration, [streams/aggregate_logs.rs:84](../src-tauri/crates/live/src/commands/streams/aggregate_logs.rs), `:130`. It allocates all regular-container source keys and sorts the full collection before truncation, `:289`. Drop standalone capped reconciliation/task bookkeeping. |
| R45. Service port-forward target selection. [service.rs](../src-tauri/crates/live/src/commands/sessions/service.rs). `select_ready_pod; resolve_service_target_port; named_container_port` | New. Bridge pure local selection. | Shuffled Pod names, readiness/deletion mixtures, missing fields and named ports found late. Check deterministic eligible Pod, skip behavior, validation and exact errors. | Retain. `service_ready_target_resolution`. Resolution runs at startup and per accepted forwarded connection, [sessions/runner.rs:130](../src-tauri/crates/live/src/commands/sessions/runner.rs), `:253`; Argo tunnels also resolve per connection, [argo/tunnel.rs:222](../src-tauri/crates/gitops/src/commands/argo/tunnel.rs). Pod selection sorts with allocating name keys, [sessions/service.rs:140](../src-tauri/crates/live/src/commands/sessions/service.rs), `:190`. |
| R46. Session snapshots, watch mapping and fanout construction. [registry.rs](../src-tauri/crates/live/src/commands/sessions/registry.rs), [registry.rs](../src-tauri/crates/live/src/commands/pod_exec/registry.rs), [watch.rs](../src-tauri/crates/live/src/commands/streams/watch.rs), [registry.rs](../src-tauri/crates/live/src/commands/streams/registry.rs). `list; stop_outside_scope local membership; event_action; StreamBroadcaster::send` | No benchmark seam needed. | No benchmark fixture justified. Preserve necessary correctness coverage. | Remove. Session lists, scope cleanup, action mapping, and watch fanout manipulate summaries/identities/subscriber handles, [sessions.rs:42](../src-tauri/crates/live/src/commands/sessions.rs), `pod_exec.rs:57`, `streams/registry.rs:67`. Polling and per-event frequency exist, but no consequential session/subscriber workload is established. Excluding aborts/IPC leaves control bookkeeping. |
| R47. Usage process-tree traversal and classification. [usage.rs](../src-tauri/crates/host/src/usage.rs), [usage_webview.rs](../src-tauri/crates/host/src/usage_webview.rs). `process_tree_pids_from_roots; selected_orphan_webview_cohort_start_time; usage_process_children` | New. Expose relation/text helpers; extract prepared-sample aggregation. | Wide/deep/shuffled trees, multiple roots, cycles/unrelated processes, platform text and ambiguous cohorts. Check exact descendants, termination, conservative attribution, CPU normalization and child ordering. | Retain. `usage_process_attribution`. The visible footer requests samples every two seconds, [AppUsageFooter.svelte:31](../src/app/svelte/AppUsageFooter.svelte). Descendant discovery repeatedly scans the complete host relation table until stable, [usage.rs:191](../src-tauri/crates/host/src/usage.rs); platform text/cohort work adds repeated allocations/scans, `usage_webview.rs:61`, `:304`. |

### Payloads

| Candidate and operation | Coverage and access | Fixture and correctness | Value decision and production evidence |
| --- | --- | --- | --- |
| R48. Complete response and stream JSON serialization. [resource.rs](../src-tauri/crates/models/src/resource.rs), [argo.rs](../src-tauri/crates/models/src/argo.rs), [rbac.rs](../src-tauri/crates/models/src/rbac.rs), [metrics.rs](../src-tauri/crates/models/src/metrics.rs), [incidents.rs](../src-tauri/crates/models/src/incidents.rs), [streams.rs](../src-tauri/crates/models/src/streams.rs). `serde_json over public DTOs` | New. Public models ready; use complete domain outputs including Events/session DTOs. | Realistic summary/topology/inspector/RBAC/metric/incident/Event/session responses and repeated stream messages. Check field/tag names, omission/unavailable values and privacy; measure Rust encoding separately from IPC. | Fold. Add attributable JSON-encoding variants to named domain/decoder fixtures. Actual Tauri encoding uses `serde_json::to_string`, installed `tauri-2.11.6/src/ipc/mod.rs:181`; `Channel::send` invokes it at `ipc/channel.rs:326`. Drop generic hand-authored DTO and small status/session-message families. |

## Worthwhile frontend workload specifications

These specifications cover all 35 retained frontend families. Cold data preparation, retained interaction updates and displayed-row preparation have separate triggers. Include folded helpers in their actual caller sequence, with production pagination, lazy consumption and output bounds. Preserve the existing useful suite stages while correcting comparator-only or uncalled paths.

1. **F01. Resource arrival and ownership projection**

   Caller: [ResourceBrowser.svelte:606](../src/features/resources/ResourceBrowser.svelte), `:644`; implementation: [resourceTableModel.ts:226](../src/features/resources/resourceTableModel.ts), [gitops-ownership-evidence.ts:117](../src/lib/gitops-ownership-evidence.ts). Dimensions are row count, searchable text, ownership depth/fanout and missing/cyclic ancestors. Each row can walk its ancestry independently. Kind filtering adds a selected-kind index and row scan.

   Gap: `benchmarks/resource-table.bench.ts:159` prepares multi-hop projection outside measurement; the measured projection at `:202` uses another fixture. Question: can repeated ancestry and ownership/search preparation share work while preserving direct-evidence precedence and identity?

2. **F02. Retained table interaction**

   Caller: [ResourceBrowser.svelte:658](../src/features/resources/ResourceBrowser.svelte); implementation: [resourceTableModel.ts:236](../src/features/resources/resourceTableModel.ts). Retain the projection outside timed search/filter/sort/page/collapse/selection operations. Cost includes full indexed filtering, sorting matching rows, grouping, selected-group reconciliation and health counting. Pagination only bounds final entries and column checks.

   Gap: existing table cases remain useful but omit several production group/preferred-Argo/selection combinations. Add the actual F29 snapshot comparison and pure serialization fixture, whose bytes scale with selected kinds/namespaces and expanded/collapsed state. Question: which full-inventory stages repeat when only page, selection or collapse changes?

3. **F04. Scope and read/watch planning**

   Callers: [ResourceBrowser.svelte:325](../src/features/resources/ResourceBrowser.svelte), `:326`, `:838`; [WorkspaceOverview.svelte:134](../src/features/workspaces/WorkspaceOverview.svelte); [CommandPalette.svelte:134](../src/app/svelte/CommandPalette.svelte); [ResourceScopeSelector.svelte:55](../src/features/resources/ResourceScopeSelector.svelte). Dimensions are namespaces, kinds/API identities, workspace contexts, overlapping watch keys and option text. Fetch products grow with namespaced kinds × namespaces; canonical keys deduplicate, sort and serialize.

   Gap: no existing suite covers this. Keep browser, workspace and palette planning as distinct fixtures. Retain prepared options for selector typing. Question: can repeated canonicalization/products be avoided without changing all-scope, cluster-scope, provider-scope or versioned-CRD identity?

4. **F05. Metric arrival and retained joins**

   Implementation: [resource-metrics.ts:16](../src/lib/resource-metrics.ts), `:29`, `:56`. Cold measurement builds one shared metric index and joins table/topology. Retained measurement starts with that index prepared. Dimensions are metric entries, resource rows, topology nodes and match density. Empty indices return existing references; matched rows/nodes allocate copies.

   Gap: `benchmarks/resource-metrics.bench.ts:69` rebuilds the shared index each iteration and uses dense matching inputs. Question: how do sparse/empty joins and retained-index updates compare while preserving unmatched identities, topology edges and warnings?

5. **F06. Table-to-map projection**

   Caller: [ResourceBrowser.svelte:675](../src/features/resources/ResourceBrowser.svelte); implementation: [resourceBrowserModel.ts:126](../src/features/resources/resourceBrowserModel.ts), `:159`, `:193`. Dimensions are filtered table rows, graph nodes/edges, ancestor closure, historical ReplicaSets and actual Pod ownership evidence. Full filtered rows drive ancestry, not the current table page.

   Gap: no direct suite. Add the missed production sequence: retained table update → historical filtering → ancestry → retained-index metric merge → consumed layout/view/fit preparation. When the map consumes the result, a fresh table model can propagate even a page/selection update through this sequence. Question: which graph preparation can remain retained without stale filtering or selection?

6. **F07. Graph normalization and layout state**

   Caller: [OwnershipMap.svelte:68](../src/features/resources/OwnershipMap.svelte); implementation: [topology-view.ts:64](../src/features/resources/topology-view.ts), [topology-implementation.ts:204](../src/features/resources/topology-implementation.ts). Dimensions are nodes, edges, roots, branch depth/fanout, duplicate/dangling identities and standalone membership. Layout state also builds a selection index, so benchmark the production wrapper as well as its substantive stages.

   Gap: existing topology fixtures mostly use Deployment/Pod pairs. Add branching, standalone collapse/expansion and network mode, including F08. Question: can duplicate graph preparation, ordering or group construction be reduced while preserving layout semantics?

7. **F09. Retained selection, focus and extent**

   Caller: [OwnershipMap.svelte:78](../src/features/resources/OwnershipMap.svelte); implementation: [topology-view.ts:78](../src/features/resources/topology-view.ts). Selection/focus changes traverse retained graph indices and map nodes/edges. Extent work also creates a node map and resolves parent positions. Resize rebuilds this view, not just a viewport scalar.

   Gap: retain `benchmarks/topology.bench.ts:139`, `:162`; broaden reachable root/group/standalone shapes. Render-visible gating at `OwnershipMap.svelte:93` bounds rendering, not these transformations. Question: can selection/focus/extent retain unaffected work without changing highlighted paths or bounds?

8. **F10. Fit-plan construction**

   Caller: [OwnershipMapViewport.svelte:32](../src/features/resources/OwnershipMapViewport.svelte); implementation: [topology-implementation.ts:460](../src/features/resources/topology-implementation.ts), `:499`. Dimensions are view nodes/edges, parent depth and selected-path size. Key construction resolves positions and sorts parts. It constructs edge parts even where the focused final key omits them.

   Gap: existing extent/view benchmarks do not call the fit-plan builder. Question: can key/bounds construction avoid unused work? Include unchanged-key inputs because the caller's equality guard runs after construction. Exclude animation, frames and `setViewport`.

9. **F12. Metadata and raw detail preparation**

   Caller: [ResourceDetailPanel.svelte:329](../src/features/resource-detail/ResourceDetailPanel.svelte); implementation: [metadata-details.ts:42](../src/features/resource-detail/metadata-details.ts), `ResourceDetailPanel.svelte:515`. Dimensions are labels, annotations, owner references, managed-field entries and nested metadata/status bytes. Curated entries are filtered/sorted; raw object values are pretty-stringified.

   Gap: no suite covers this. Four visible badges, `metadata-details.ts:33`, do not bound upstream preparation. Native collapsed raw metadata still contains its row loop. Question: can repeated formatting/scans be reduced without losing raw detail or curated ordering?

10. **F13. Stream append and retained log search**

    Caller: [ResourceLogsPane.svelte:107](../src/features/resource-detail/ResourceLogsPane.svelte), `:180`; implementation: [log-helpers.ts:28](../src/features/resource-detail/log-helpers.ts), `:54`, `:72`. Append parses the incoming line; retention caps the buffer at 1,000 lines, `log-helpers.ts:18`. Nonempty search scans retained searchable bytes; latest-first clones/reverses only when its dependencies change.

    Gap: retain append/eviction at `benchmarks/resource-detail.bench.ts:97`. Replace `:107`, which uses uncalled `orderedLogLines`, with prepared-line search/order updates. Include timestamp presentation only for mounted matches. Question: can repeated normalization/order allocation be reduced within the real bound?

11. **F14. Event arrival and retained filtering**

    Callers: [ResourceDetailPanel.svelte:341](../src/features/resource-detail/ResourceDetailPanel.svelte), [EventsTab.svelte:44](../src/features/resource-detail/EventsTab.svelte), `:45`; implementation: [eventsTabModel.ts:41](../src/features/resource-detail/eventsTabModel.ts), `:79`. Dimensions are events, message/source bytes, type selectivity and timestamp availability. Data arrival sorts and summarizes; search/type changes filter retained sorted events.

    Gap: retain the cold case at `benchmarks/resource-detail.bench.ts:113`; add retained filtering and mounted-row timestamp preparation. `EventsTab.svelte:189` renders all filtered events, without a page cap. Question: can repeated field normalization and timestamp formatting be retained while preserving summary/filter semantics?

12. **F15. Detail signals and timeline presentation**

    Callers: [ResourceDetailPanel.svelte:321](../src/features/resource-detail/ResourceDetailPanel.svelte), `:348`, `:357`; [ResourceTimeline.svelte:40](../src/features/resource-detail/ResourceTimeline.svelte). Dimensions are conditions, container-status entries, events, retained log lines and output timeline entries. Cold detail preparation includes F11. Timeline construction scans signals and sorts entries; latest-log selection sorts timestamped retained logs.

    Gap: existing timeline benchmark starts with prepared conditions/containers and stops before date grouping/title/time preparation. Grouping responds to entries/timezone; midnight changes heading state, not the entry groups. Expansion does not reparse details. Question: can parsed timestamps, repeated titles and grouping remain retained? Hidden-tab consumption must remain explicit.

13. **F16. YAML comparison**

    Callers: [ResourceYamlPane.svelte:134](../src/features/resource-detail/ResourceYamlPane.svelte), [DeploymentRevisionsTab.svelte:64](../src/features/resource-detail/DeploymentRevisionsTab.svelte); implementation: [yamlTabDiff.ts:34](../src/features/resource-detail/yamlTabDiff.ts), `:145`. Dimensions are input lines/bytes, edit distribution/repeated blocks and requested context. Compact/full and git/clean branches have different output work.

    Gap: retain `benchmarks/yaml-diff.bench.ts`; extend active style/context/identical/newline branches. The preview's 24-line slice does not bound diff construction. Revision selection uses the shared compact diff rather than another substantive algorithm. Question: can comparison/output preparation avoid work discarded by presentation without changing diff direction/context?

14. **F18. Secret document parsing and chosen-value decoding**

    Caller: [SecretDataViewer.svelte:12](../src/features/resource-detail/SecretDataViewer.svelte), `:37`; implementation: [secretData.ts:16](../src/features/resource-detail/secretData.ts), `:41`. Document parsing builds an AST, converts to JSON and projects `data`/`stringData`. Decoding validates base64 and allocates bytes/text for one selected key.

    Gap: no benchmark covers either stage. Dimensions are whole YAML bytes/nesting/key count and the chosen encoded value's bytes, independently. Question: can parsing/conversion or decoding allocations shrink while preserving masks, invalid input, binary/UTF-8 behavior and error semantics? Use synthetic secrets only.

15. **F19. Explicit document formatting**

    Caller: [ResourceYamlPane.svelte:382](../src/features/resource-detail/ResourceYamlPane.svelte); implementation: [yamlFormat.ts:17](../src/lib/yamlFormat.ts), `:48`. The formatter validates via `parseDocument`, then parses again before YAML or KYAML serialization. Dimensions are bytes, nesting, collection width and scalar quoting.

    Gap: no existing formatter benchmark. Question: can validated parse work be reused without changing duplicate-key/error behavior, comments/style decisions or serialized values? Include valid and rejected documents; do not model formatting as a keystroke operation.

16. **F21. Lint result production and application**

    Caller: [YamlCodeEditor.svelte:103](../src/components/YamlCodeEditor.svelte); returned-field mapper: [ResourceYamlPane.svelte:359](../src/features/resource-detail/ResourceYamlPane.svelte). Dimensions are document bytes/lines, returned diagnostic count, key location/missing keys and diagnostics per line. `findYamlFieldRange` splits/scans the document per diagnostic, giving a diagnostic-count × document-work shape.

    Gap: no suite covers the full local/prepared-result/lens stages. F23 skips informational messages, deduplicates lines and maps retained ranges on transactions. Question: can one line/key preparation serve multiple diagnostics and can lens ranges stay retained? Exclude Rust lint IPC, debounce time and DOM widgets.

17. **F22. Editor decoration updates**

    Caller: [YamlCodeEditor.svelte:129](../src/components/YamlCodeEditor.svelte); implementation: [yamlCodeViewerExtensions.ts:26](../src/components/yamlCodeViewerExtensions.ts), `:164`. Dimensions are bytes, lines, indentation depth and value/comment token spans. Every document change rebuilds full decorations; non-document transactions retain the set.

    Gap: no executed headless benchmark exists. Question: can small edits avoid whole-document decoration reconstruction while preserving exact spans and ordering? The production update listener also serializes the whole document at `YamlCodeEditor.svelte:119`, and synchronization serializes it at `:149`. Record that separate edit-path cost rather than silently attributing it to decorations.

18. **F26. Shallow sidebar and expanded namespace updates**

    Callers: [SidebarTree.svelte:90](../src/app/svelte/SidebarTree.svelte), `:149`, `:180`; [SidebarTreeNode.svelte:39](../src/app/svelte/SidebarTreeNode.svelte), `:43`. Dimensions are namespaces, custom kinds/API versions, search text, expanded IDs and present kinds in each expanded namespace. Expanded membership uses array inclusion; result/error/pending maps traverse expanded query results separately.

    Gap: retain shallow/full-tree suite stages. The eager-child case represents an all-expanded fixture only, not normal sidebar behavior. Use actual per-namespace present-kind subsets. Question: can search/tree grouping and expanded membership/maps share preparation without constructing collapsed children or mismatching query results?

19. **F31. Workspace health and selected comparison**

    Caller: [WorkspaceOverview.svelte:186](../src/features/workspaces/WorkspaceOverview.svelte), `:239`; implementation: [workspace-model.ts:284](../src/lib/workspace-model.ts), `:476`, `:504`. Dimensions are loaded rows, health/restart evidence and selected partition membership. Comparison entries are limited by production to two, followed by filtering and health reductions.

    Gap: no suite covers this. Question: can aggregate health and the two comparisons share row work without changing overlap, scope or missing-resource semantics? Avoid synthetic all-pairs workloads.

20. **F32. Authoritative entry reconciliation and changed-entry persistence**

    Caller: [WorkspaceOverview.svelte:181](../src/features/workspaces/WorkspaceOverview.svelte); implementation: [workspace-entry-points.ts:137](../src/lib/workspace-entry-points.ts), [workspaceStore.ts:230](../src/features/workspaces/workspaceStore.ts). Dimensions are live rows, pinned entries, coverage plans/requests and persisted collection bytes. Recent entries are capped at eight, `workspace-entry-points.ts:3`; pins are not given that bound.

    Gap: no suite covers live-key construction, per-entry nested coverage checks, normalization/equality or whole-collection serialization after a change. Question: can coverage indexing and normalization be shared while removing only proven covered absences? Preserve unchanged-update short-circuit; isolate clock/storage from measured work.

21. **F36. Palette cache harvest and cold search index**

    Caller: [CommandPalette.svelte:168](../src/app/svelte/CommandPalette.svelte); implementation: [entries.ts:67](../src/features/command-palette/entries.ts), `:170`. Dimensions are retained cache queries, matching cache sets, total rows, duplicates, unique identities and search-text bytes. Production prioritizes warmed rows, then Argo rows, then cached rows.

    Gap: existing palette suite retains the useful one-pass dedupe/index case but starts with arrays. Add the missed `getQueriesData` harvest and `cached.map` preparation. Cache access is an imperative snapshot, not a subscription proving that every cache update rebuilds the index. Question: can harvest/deduplication avoid repeated copies while preserving first-wins identity?

22. **F37. Palette typing**

    Caller: [CommandPalette.svelte:163](../src/app/svelte/CommandPalette.svelte), `:180`. Retain resource index and namespace data outside timed query updates. Dimensions are indexed searchable bytes, namespace text and hit selectivity. Both filters scan before slicing to production caps.

    Gap: no standalone retained filtering case. Question: can capped result production stop earlier while preserving order and the actual blank-query behavior? Do not rebuild Argo/cache/index preparation for each character.

23. **F39. RBAC cold cockpit and retained search**

    Caller: [RbacView.svelte:180](../src/features/rbac/RbacView.svelte), `:181`; implementation: [cockpitModel.ts:34](../src/features/rbac/cockpitModel.ts), `:133`, `:193`. Dimensions are roles, bindings/subjects, service accounts/groups, coverage and rule/search text. Cold shaping constructs indices/risk projections and sorts; retained search rebuilds searchable strings.

    Gap: retain `benchmarks/major-surfaces.bench.ts:278`, but split its cold/search stages and include active categories. The visible-row limit does not bound filtering. Question: can stable searchable text and risk evidence be retained between inputs without stale results?

24. **F40. Observed permission join**

    Caller: [RbacView.svelte:221](../src/features/rbac/RbacView.svelte); implementation: [observedPermissions.ts:21](../src/features/rbac/observedPermissions.ts), `:88`. Dimensions are roles, bindings × subjects, matched rules and duplicate grant origins. It builds role/coverage maps, matches identity/group membership, serializes grant keys and merges/sorts origins.

    Gap: no existing join benchmark. Question: can stable inspection indices and canonical grant preparation survive identity edits while preserving scope, automatic service-account groups, duplicate-role behavior and origin ordering?

25. **F41. Reviewed-row evidence**

    Callers: [RbacReviewBadge.svelte:21](../src/features/rbac/RbacReviewBadge.svelte), [RbacReviewPanel.svelte:40](../src/features/rbac/RbacReviewPanel.svelte), `:52`; implementation: [reviewModel.ts:31](../src/features/rbac/reviewModel.ts), `:138`, `:161`, `:234`. Dimensions are review records, mounted reviewed rows, relevant bindings, role lookups and canonical evidence bytes. Hashing uses per-character BigInt work after canonical sorting/serialization.

    Gap: no suite covers record lookup, reviewed-row hashes or absent-review fast paths. Existing keyed rows with unchanged inputs are not proven to rehash on every search. Newly mounted reviewed rows, inspection changes and review changes matter. Question: can shared canonical evidence/indices reduce repeated selected/queue/panel work without accepting stale reviews?

26. **F42. RBAC queue materialization**

    Caller: [RbacView.svelte:416](../src/features/rbac/RbacView.svelte), `:420`, `:465`, `:725`. Dimensions are mounted queue rows, full bindings/subjects and cockpit inventory. A visible Service Account's tag reconstructs/scans the binding collection; selected sources repeat the helper. Bucket labels filter the unsearched inventory.

    Gap: current cockpit benchmark excludes these displayed-row costs. Use the caller's 50-row initial/load-more behavior, not every RBAC object by default. Question: can cold binding indices serve tags/sources and bucket counts while preserving group grants and source identities?

27. **F44. Helm release surface**

    Caller: [HelmSurface.svelte:114](../src/features/helm/HelmSurface.svelte); implementation: [surfaceState.ts:4](../src/features/helm/surfaceState.ts); counts: [HelmView.svelte:105](../src/features/helm/HelmView.svelte). Dimensions are releases, namespace groups, searchable field bytes and reconciliation resources. Search reconstructs joined lowercase text and also regroups the complete release list. Counts are data-only; reconciliation sorting is separately data-driven.

    Gap: existing case at `benchmarks/major-surfaces.bench.ts:291` omits production search and includes uncalled `resourcesOwnedByHelmRelease`. Retain active grouping/reconciliation fixtures, replace that leg. Question: can grouping/search preparation remain retained between text inputs?

28. **F45. GitOps inventory and displayed rows**

    Callers: [GitOpsSurface.svelte:144](../src/features/gitops/GitOpsSurface.svelte), `:156`, `:160`, `:161`; [GitOpsView.svelte:129](../src/features/gitops/GitOpsView.svelte), `:499`, `:662`. Dimensions are provider inventories, Flux kinds, assessed health, selected inventory rows and sources/revision text per row. Rails are data-driven; table/selections/summary also respond to active rail/kind.

    Gap: retain `benchmarks/major-surfaces.bench.ts:283`, add read-state/selections and assessed Argo health, which legacy `healthStatus` fixtures do not cover. F46 repeatedly groups/scans sources for eagerly prepared row labels/tooltips. No row pagination protects this path. Question: can row source/revision preparation share scans while preserving all fallbacks and provider-wide summaries?

29. **F47. Managed-resource inventory matching**

    Callers: [ResourceBrowser.svelte:615](../src/features/resources/ResourceBrowser.svelte), `:627`; [ArgoApplicationDetails.svelte:257](../src/features/gitops/ArgoApplicationDetails.svelte), `:339`; implementation: [argo-workspace-model.ts:33](../src/features/gitops/argo-workspace-model.ts), `:66`. Dimensions are managed resources, workspace rows, filter selectivity and identity validity/duplicates. Counts use multiple passes; filtering builds managed identity sets and scans workspace rows.

    Gap: no suite covers the join. Question: can counts/identity preparation share work across filter changes while preserving API-group precedence, core identities, ordering and unchanged-reference returns?

30. **F48. Argo inspector document display**

    Caller: [ArgoApplicationDetails.svelte:912](../src/features/gitops/ArgoApplicationDetails.svelte); helpers: `:577`, `:610`, `:616`, `:621`, `:642`. Dimensions are displayed reconciliation resources, comparison records, document bytes, edits and highlighted output lines. Lookup uses linear matching; row badges can repeat lookup/serialization. Selected connected comparison overrides still perform the eager lookup first.

    Gap: no suite covers the actual display sequence. Include F49's YAML fallback once per changed application YAML. Collapsed removal rows skip document diff/highlighting, while their row lookup/document selection still occurs. Question: can row lookup/serialization be shared without breaking authoritative flags, comparison precedence or lazy document branches?

31. **F50. Operation observation and progress display**

    Caller: [argo-operation-tracker.ts:115](../src/features/gitops/argo-operation-tracker.ts); implementation: [argo-operation-progress.ts:33](../src/features/gitops/argo-operation-progress.ts), `:71`. Dimensions are operation resource/hook rows and message bytes. Each observed result projects progress and outcome; sync outcome projects progress internally, and display projects progress again. This is concrete repeated array/redaction work.

    Gap: no suite covers this sequence. Question: can one prepared progress result serve outcome/display without changing fresh-observation rules, terminal detection, refresh semantics or redaction? Benchmark computation on prepared observations; exclude polling and transport.

32. **F51. Incident queue interaction**

    Caller: [IncidentSurface.svelte:85](../src/features/incidents/IncidentSurface.svelte); implementation: [model.ts:164](../src/features/incidents/model.ts). Dimensions are incident items/signals, matching items, groups and timestamp ties. Every model call counts full inventory, filters/sorts/groups and reconciles selection. A selected-key change invokes the same operation.

    Gap: no suite covers this. Question: can selection-only changes and filter updates retain counts/group preparation without changing overlapping warning/restart categories, order or disappearing-selection behavior?

33. **F52. Selected incident enrichment**

    Callers: [IncidentSurface.svelte:232](../src/features/incidents/IncidentSurface.svelte), `:238`; implementation: [incident-actions.ts:44](../src/features/incidents/incident-actions.ts), [guidance.ts:109](../src/features/incidents/guidance.ts), `:124`. Dimensions are namespace graph nodes/edges, owner depth/branching, snapshot signals, conditions and containers. Owner resolution builds full graph indices before BFS. Evidence scans/deduplicates inputs before the six-entry output slice.

    Gap: no suite covers this. Use separate topology-arrival and prepared-owner/detail-arrival fixtures, incorporating F53. Topology query enablement is for the selected namespaced Pod, not every incident. Question: can graph/evidence preparation be reused without choosing an incorrect owner or presenting unsupported actions?

34. **F54. Diagnostics refresh and report preparation**

    Callers: [DiagnosticsSettings.svelte:64](../src/app/svelte/DiagnosticsSettings.svelte), `:126`; implementation: [diagnostics.ts:108](../src/lib/diagnostics.ts), `:159`, `U/diagnostics-report.ts`. Dimensions are actual retained events, metric groups, trace bytes and backend report records. Frontend buffers are bounded to 500 events and 400 trace lines, `diagnostics.ts:44`. Snapshot refresh groups/sorts durations every 1,500 ms while mounted.

    Gap: no suite covers periodic snapshot/table preparation or explicit redacted-report serialization. Question: can repeated summary/report preparation avoid redundant sorting/copies within these bounds? Keep report creation distinct from periodic refresh and fix clock/state inputs.

35. **F58. Session refresh and saved-forward display**

    Callers: [LiveSessionsSurface.svelte:59](../src/features/live-sessions/LiveSessionsSurface.svelte), `:76`; [LiveSessionsView.svelte:303](../src/features/live-sessions/LiveSessionsView.svelte). Implementations: [portForwardLifecycle.ts:120](../src/features/live-sessions/portForwardLifecycle.ts), [podExecLifecycle.ts:33](../src/features/live-sessions/podExecLifecycle.ts), [liveSessionReadModel.ts:24](../src/features/live-sessions/liveSessionReadModel.ts). Dimensions are returned sessions, workspace contexts, visible sessions and saved presets. Query arrival sorts each type; projection filters, combines, sorts again and splits. Saved rows perform first-match searches across visible forwards.

    Gap: no suite covers these repeated stages or F59 matching. Question: can sorted/session identity preparation be shared while preserving source/context/status semantics and first-match behavior? Conflict checks belong to start actions. Exclude session I/O.

### Existing suites to retain and correct

| Suite | Retain | Correction or missing production coverage |
|---|---|---|
| `benchmarks/resource-table.bench.ts` | Projection and retained interaction stages | Time multi-hop projection itself; extend group/selection fixtures. |
| `benchmarks/topology.bench.ts` | Layout state, indexed selection and retained view | Full-rebuild wrappers are comparison/harness paths, not ordinary production selection. Add branching, modes and fit plans. |
| `benchmarks/resource-detail.bench.ts` | Append/eviction, cold event processing, timeline assembly | Replace uncalled retained-log reparse path; add retained filtering and presentation. |
| `benchmarks/sidebar-tree.bench.ts` | Shallow tree, complete tree, reachable expanded-child stages | Label eager children as all-expanded; use per-namespace present kinds and exclude fixture-only input construction. |
| `benchmarks/major-surfaces.bench.ts` | RBAC cold shaping, GitOps rails/summary/table, active Helm grouping/sorting | Separate retained inputs; add assessed Argo health; replace uncalled Helm owned-resource leg. |
| `benchmarks/command-palette.bench.ts` | Production one-pass dedupe/index | Flatten comparator is not the active caller. Add cache harvest and retained typing. |
| `benchmarks/resource-metrics.bench.ts` | Shared-index cold join | Duplicate-index case is a comparator. Add retained-index and sparse/empty matches. |
| `benchmarks/yaml-diff.bench.ts` | Active compact/full diff stages | Expand actual style/context branches and presentation-boundary fixtures. |

## Worthwhile Rust workload specifications

These specifications cover the complete retained Rust workload set, not a starter selection. IDs point to the source-linked audit tables above. Separate variants expose attribution without multiplying small helper benchmarks. Fixture sizes come from existing fixtures, recorded shapes or explicit production bounds. Network, OS sampling and IPC waits stay outside CodSpeed.

1. **R01/R04, typed and dynamic list projection.** Prepared fetched collections → complete summaries, including R02/R03. Dimensions are rows, relevant metadata string bytes, owner references, Pod container statuses, and recognized conditions. Reuse the existing normal/large Pod shape in `e2e/harness/memory-fixture.ts:24`; add per-kind branches without inventing expansion factors. Include competing ownership evidence and terminal/missing status. Coverage gap: No backend summary coverage. Optimization question: Can duplicate ownership construction, readiness parsing, evidence allocation, and identity copies be reduced across complete batches?

2. **R05/R06, scope assembly and Flux index construction.** Keep two measurements: fetched owner objects → index, and completed resource batches plus cached index → scope filtering/enrichment/deduplication. Dimensions are retained owner entries, identity bytes, requested namespaces, rows, duplicates, and ownership hits. Preserve the existing owner/inventory bounds at [helpers/metadata/flux_inventory.rs:10](../src-tauri/crates/kube/src/commands/helpers/metadata/flux_inventory.rs). Coverage gap: Both stages are uncovered. Optimization question: Can parser intermediates, repeated namespace scans, and per-row lookup-key copies be reduced? R07 belongs here, with a topology-specific variant for its temporary summary copies.

3. **R08/R09, ownership and network topology.** Maintain existing ownership and CRD-overlay inputs, then add complete production mapping variants containing port hints, labels, health, and Flux ownership. Add the StatefulSet/Pod/PVC relationships represented by `e2e/fixtures/gitops/tenants/ledger/workload.yaml:12`, plus missing-owner/namespace-collision cases. Network fixtures must exercise EndpointSlice precedence and selector fallback. Dimensions are resources, owner edges, claims and StatefulSet Pods, Services, slices, Pod targets, selector fields, and duplicate edges. Coverage gap: Existing fixtures omit PVC/network branches and production mapping. Optimization question: Can full-node reconstruction, per-claim scans, and Service×slice/Pod scans be reduced? Retain graph-only attribution alongside complete local preparation.

4. **R11, cache payload reuse and replacement.** Prepare cache/runtime outside timing. Measure exact ready resource hits, all-namespace-to-named-scope reads, topology/Flux-index hits, and watch-dirty replacement with an immediate prepared owned result. Include R12/R14/R15 in the replacement/read paths. Dimensions are actual payload rows/nodes/evidence/string bytes, retained fraction, namespace set, and cache state. Coverage gap: Existing cache diagnostics do not measure payload copying. Optimization question: Can whole-payload copies and discarded-row copies be reduced while preserving owned outputs, scope isolation, and loader behavior?

5. **R17, CRD catalog construction.** Prepared CRDs → served-version selection, catalog DTOs, sorting, and deduplication. Maintain the existing catalog-sort fixture and add projection over corresponding CRD shapes. Dimensions are CRDs, served versions, short names, identity bytes, and duplicates. Fold Namespace age/name projection into R01. Coverage gap: Current measurement starts with prepared kinds. Optimization question: Which projection copies and sort/deduplication costs affect the complete catalog load?

6. **R18, kubeconfig parsing/merge/context projection.** Credential-free source text → the installed library parser and merge → context summaries. Dimensions are sources/documents, accumulated named entries, collisions, field bytes, and extensions. Installed `kube-client-4.2.0/src/config/file_config.rs:582` rebuilds a HashSet of accumulated names and an intermediate Vec during each named-list merge. Coverage gap: Uncovered. Optimization question: How much repeated decoding and accumulated-list rebuilding occurs? Exclude disk/environment access and relative-reference remapping. Fail-closed validation is the repeated production trigger, not ordinary ready client lookup.

7. **R20/R21, documents and complete inspection details.** Prepared typed/dynamic workloads, ConfigMaps, CRDs, and Secrets → actual conversion, normalization/redaction, encoding, and detail DTO assembly. Dimensions are tree nodes/depth, spec/status/managedFields bytes, strings, and Secret keys. Reuse the existing 40-container document. Coverage gap: Current YAML/KYAML cases measure emission from prepared JSON only. Optimization question: Can recursive string assembly, duplicate conversion, and metadata/status materialization be reduced? Include the current discarded first Secret serialization. Typed `get_resource_details` currently ignores mode/encoding arguments at [resources/details.rs:155](../src-tauri/crates/inspection/src/commands/resources/details.rs); fixtures must reflect that behavior.

8. **R22, Deployment revision history.** Prepared candidate ReplicaSets plus exact Deployment UID → ownership filter, complete template YAML, revision DTOs, and sorting. Dimensions are candidates, accepted revisions, owner references, template bytes, and ties. Coverage gap: No revision/template-pipeline coverage. Optimization question: Can emission and output assembly allocate less while preserving all revisions and exact ordering? Exclude Deployment GET and ReplicaSet LIST.

9. **R23/R24, editor local work.** Use separate `editor_local_lint`, `editor_apply_validation`, and captured `editor_schema_output_projection` variants. Dimensions are draft bytes/depth/document structure and captured schema-error/message/pointer bytes. Include invalid/multiple documents, target mismatch, Secret rejection, and valid/invalid/skipped/error schema outcomes. Coverage gap: Formatter and frontend diff cases omit parsing/validation. Optimization question: Can parsing/conversion and unused-error decoding allocate less without weakening validation? Do not invent multi-resource editor batches.

10. **R25/R27, Helm storage and manifest parsing.** Encoded Secret/ConfigMap records → decoding, safe metadata/value summaries, manifest parsing/preview, latest selection, and consumed-record destruction. Encode fixtures before timing. Dimensions are stored/expanded bytes, revisions, release identities, manifest bytes/documents, and config keys. Preserve production's 1 MiB storage and 32 MiB decoded bounds, [helm/storage.rs:29](../src-tauri/crates/gitops/src/commands/helm/storage.rs). Maintain a separate manifest-parser signal over mixed documents and embedded content. Coverage gap: Existing minimal Deployment parser fixtures omit full storage and substantial document content. Optimization question: Can expensive work for discarded revisions or full YAML materialization for identity extraction be avoided while preserving decoded identity precedence, errors, and document order?

11. **R28, Helm reconciliation.** Measure discovery/manifest preparation and prepared-result assembly as separate phases around network waits. Dimensions are discovered kinds, manifest refs, label-only rows, duplicate identities, unavailable outcomes, and warnings. Coverage gap: Existing Helm parser and frontend grouping cases do not cover these joins. Optimization question: Can borrowed identity keys and comparisons reduce index/sort cloning while preserving manifest precedence and deterministic output?

12. **R29, Argo list projection.** Separate Application, ApplicationSet, and AppProject batches. Dimensions are objects, tracked resources/distinct namespaces, sources/revisions, condition evidence, and pending-operation state. Coverage gap: Frontend GitOps benches start with DTOs. Optimization question: Can repeated source/revision/namespace copies and health allocation be consolidated? Do not inflate AppSet generator arrays as a cost dimension.

13. **R30/R31, Argo inspector and selected comparison.** Prepared Kubernetes Application or connected response bodies → production status/history/resource projection, state decoding, and redaction. Selected-comparison variants must follow their own Kubernetes and connected paths. Dimensions are resources/history/conditions, total tree/state bytes, state representation, Secret density, and match position. Include object, JSON, YAML, malformed, and absent states. A captured response-byte decode variant can include the real `serde_json::from_slice` stage at [argo/transport.rs:58](../src-tauri/crates/gitops/src/commands/argo/transport.rs). Coverage gap: Uncovered. Optimization question: Can redundant tree clones, redaction traversals, and allocating identity lookups be reduced? Exclude HTTP/action-request waiting.

14. **R32, Argo identity revalidation.** Reuse substantial R30 Application objects with matching and stale reviewed identities → `verify_kubernetes_application_identity`. Dimensions are complete Application bytes and expected identity fields. Coverage gap: Uncovered. Optimization question: Can direct field access avoid full-object serialization while preserving every rejection? Payload-option helpers are not independent targets.

15. **R35/R34, Flux list projection.** Prepared source/Kustomization/HelmRelease objects plus discovery → complete list summaries. Dimensions are objects, condition position, inventory entries/bytes, malformed entries, and discovered APIs. Preserve prefix-before-filter behavior and existing inventory bounds. Coverage gap: Dynamic-summary and frontend GitOps coverage omit the Flux DTO/inventory pipeline. Optimization question: Can repeated Ready extraction and string ownership be consolidated? Exclude the currently uncalled details-only `bounded_status` path.

16. **R36/R37, RBAC inventory projection and rollup.** Prepared Kubernetes inventory and family coverage outcomes → all role/binding/subject projections, risk inheritance, and namespace rollups. Production uses inferred namespace buckets, not an invented explicit namespace list. Dimensions are roles/bindings/rules/subjects/namespaces, reference position/misses, duplicates, and coverage. Coverage gap: No backend RBAC coverage. Optimization question: Can role indexing and fewer normalized-set, identity, subject, and risk copies reduce complete assembly without changing coverage-sensitive unknown risks or output order?

17. **R39/R38, metrics normalization and rollup.** Prepared Pod/Node DynamicObject metrics plus Pod/ReplicaSet inventories → quantities, container totals, and workload contributions. Existing `e2e/harness/memory-fixture.ts:41` supplies a matching metrics shape. Dimensions are metric objects, containers, quantity bytes/units, owner references, workload groups, missing joins, and source-Pod contributions. Coverage gap: Frontend merging omits backend normalization/rollup. Optimization question: Can borrowed quantity strings and fewer tuple-key copies reduce work while preserving overflow, timestamps, parent contributions, and source-Pod order?

18. **R40/R41, incidents and selected-resource Events.** Keep distinct pipelines. Incident fixtures include raw warning Events, resource summaries, and fetched Pods through restart extraction, warning shaping, joins, and final ordering. Event fixtures start with the server-filtered selected-resource Event collection. Dimensions are returned Events, resources, container statuses, retained signals, evidence/message bytes, UID matches, and timestamp ties. Preserve existing incident ceilings at [incidents.rs:31](../src-tauri/crates/inspection/src/commands/incidents.rs). Coverage gap: Frontend timelines omit backend joins and timestamp conversion. Optimization question: Can parsed timestamp keys and fewer temporary evidence collections reduce work without changing historical evidence or stable-sort-then-reverse behavior?

19. **R42/R43, log and exec byte processing.** Measure complete prepared byte sequences, not one scalar decoder call. Log work includes buffer-spanning lines, LF/CRLF/EOF, invalid UTF-8, exact retention-boundary cases, and oversized-line draining followed by another line. Exec work appends actual read-sized chunks, exhausts prefixes, preserves incomplete bytes, and flushes EOF. Dimensions are bytes, segmentation, line lengths, multibyte splits, and invalid sequences. Production bounds are 256 KiB retained per log line and a 4,096-byte exec read buffer. Coverage gap: No backend decoder coverage. Optimization question: Can scanning/buffer growth or repeated drain movement improve without weakening retention, yielding, errors, or replacement semantics? Exclude network readiness and IPC.

20. **R44, aggregate-log source enumeration.** Prepared matching Pods → regular-container sources, complete count, sorting, and production truncation. Dimensions are Pods, regular containers, source-key bytes, deletion/malformed fractions, and input order. Coverage gap: Uncovered. Optimization question: Can per-Pod intermediate Vec allocation or full sorting be reduced while preserving selected keys and pre-truncation totals? The existing 50-active-source cap does not bound enumeration input.

21. **R45, Service ready-target resolution.** Prepared owned Pod collection plus selected ServicePort → complete numeric/named target selection. Dimensions are selector-matching Pods, readiness conditions, names, container ports, and eligible/port-bearing match positions. Coverage gap: Uncovered despite repetition per accepted connection. Optimization question: Can deterministic selection avoid full sorting and repeated name-key allocation while preserving skip behavior, validation, and exact errors? Reset consumed inputs outside timing; exclude API/socket work.

22. **R47, usage process attribution.** Prepared host relations, platform identity/start-time fields, and usage samples → descendants, applicable orphan attribution, totals, and ordered breakdown. Dimensions are host process count, reachable descendants, depth/order, candidate cohorts, command-line bytes, and selected children. Coverage gap: Uncovered. Optimization question: Can adjacency traversal and identity-text reuse reduce repeated full-table passes while preserving conservative attribution? Use recorded process-table shapes; exclude `sysinfo` OS refresh and sampling waits.

23. **R48, domain JSON encoding variants.** Encode complete outputs from R01/R04/R08/R09/R30/R36/R39/R40/R41 and actual decoded R42/R43 messages with the production JSON String representation. Dimensions are output elements, nested nodes, escaped string bytes, and emitted messages. Coverage gap: Existing YAML emission and topology construction do not measure response encoding. Optimization question: Which domain changes add encoding traversal/copy/allocation work? Keep encoding attributable, with IPC and webview delivery outside timing.


Of the nine existing Rust cases, six provide independent bulk-work signals: manifest parsing, both YAML encodings, both ownership-topology variants and catalog sorting. The implementation retains those six, folds bounded manifest-preview and namespace-key coverage into their production workloads, and removes the cache-weight diagnostic timing. That case read shallow counts rather than traversing its 500-app fixture; cache benchmarks now measure actual payload reuse and replacement.

## Evidence that benchmark work matters

The [recorded frontend measurements](frontend-memory-retention.md#historical-90-second-deterministic-fixture-observations) include a 10,000-row fixture with projection at 19.55 ms, metrics merging at 6.83 ms, and grouped topology construction at 66.29 ms. Name-sort changes took 34.30, 18.37 and 17.43 ms. These are historical local observations, not current CodSpeed results, regression thresholds or a cluster-size recommendation. The raw artifacts are absent from this checkout. They support retaining these CPU workloads; they do not attribute native rendering or process memory to those functions.

The production [resource browser](../src/features/resources/ResourceBrowser.svelte) separates data preparation at lines 603-644 from interaction shaping at lines 658-673. Scope filtering, metrics joining and ownership/search projection react to their data dependencies. Search, sorting, paging, collapse and selection reuse the projection. Benchmark both operations with their actual retained state. Do not charge cold projection construction to every keystroke. The topology branch at lines 675-685 depends on filtered table rows, so a table interaction can also cause topology filtering and metrics joining when that branch is consumed.

The [historical Rust profile comparison](release-footprint-variants.md#existing-backend-hot-paths) records ownership graph construction at 3.39 ms, CRD-overlay construction at 5.578 ms and Helm manifest parsing at 1.025 ms in its baseline. Those fixtures already establish substantive local work. By comparison, shallow cache-weight inspection was 129.8 ns and bounded Helm preview summarization was 3.556 microseconds. Preserve existing coverage, but these latter observations do not justify expanding diagnostics and tiny preview bookkeeping into independent optimization projects.

[Ready cache reads](../src-tauri/crates/kube/src/commands/live_store.rs) clone values at lines 102 and 123. A warm read therefore has payload-size-dependent CPU/allocation cost despite making no cluster request. The recorded native fixture's resource and topology JSON estimates were 7,300,815 and 13,937,573 bytes. JSON bytes are not heap size or clone cost, but they demonstrate that large production-shaped response fixtures already exist. Large resource/topology response encoding is also worthwhile: Tauri's installed `IpcResponse` implementation serializes returned `Serialize` values with `serde_json::to_string`. Measure that local encoding with the actual response models; keep channel transfer and webview decoding in diagnostics.

[Log retention](../src/features/resource-detail/log-helpers.ts) is bounded to the production 1,000-line buffer, yet every overflowing append shifts the array. [The stream consumer](../src/features/resource-detail/ResourceLogsPane.svelte) parses incoming lines once and reuses parsed entries for search/reversal. [The detail timeline](../src/features/resource-detail/ResourceDetailPanel.svelte) consumes those entries; its [latest-log helper](../src/features/resource-detail/log-helpers.ts) filters and sorts timestamps. These repeat-triggered operations deserve steady-state cases at the existing bound, not invented unbounded log fixtures.

## Measurement choices

The current workflow uses CPU simulation. It estimates instruction and cache/memory costs, and excludes time spent in system calls. Use it for local parsing, indexing, matching, graph construction, sorting, serialization, cloning and deterministic state transformations. Its estimated time does not establish application latency. [CodSpeed CPU simulation](https://codspeed.io/docs/instruments/cpu)

An optional memory lane can reuse allocation-heavy candidates, especially summaries, topology, YAML, metrics, Helm decoding and cache hits. The pinned Rust and Vitest integrations meet the documented minimum versions. Rust needs a memory-mode build. Allocation measurements and peak process memory still do not identify retained JavaScript objects or prove native WebView savings. Allocator, kernel and concurrent-thread limitations apply. No memory lane was added in this audit. [CodSpeed memory instrument](https://codspeed.io/docs/instruments/memory)

V5 allocator exclusion is opt-in, and this workflow does not enable it. Keep measurement mode, toolchain, features and allocator treatment consistent across comparisons. Earlier simulation-version results are not comparable to V5 results. [CodSpeed V5 release notes](https://codspeed.io/changelog/2026-08-03-codspeed-v5-only-your-code-at-its-real-cost)

Use Settings -> Diagnostics for real Tauri, cluster and network latency. Use the existing native profiling paths for startup, browser layout/paint, graph interaction, editor responsiveness and process memory. Local walltime observations supplement simulation; they do not replace its regression signal. A future CodSpeed walltime lane needs stable runner hardware and representative local fixtures. Do not run competing walltime benchmark processes on the same machine. [CodSpeed walltime](https://codspeed.io/docs/instruments/walltime)

## Boundaries and exclusions

Use live diagnostics or native application profiling for Tauri/Kubernetes/Argo/updater calls, Secret reveal, session startup, watches/recovery/cancellation waits, storage/history writes, module loading, Svelte mounting/layout/scrolling/tooltips, SvelteFlow paint/culling/animation, CodeMirror rendering, xterm fitting/output paint and cross-platform WebView retention. CPU-only fixtures cannot prove these operations became faster.

The inspected paths below do not justify standalone production benchmarks:

- Uncalled resource helpers `sortedRows`, `filterResources`, `filterTopologyByKinds`, `createResourceTableState`; detail `formatMetadata`; legacy RBAC surface builders; standalone Argo status insights; GitOps overview filters; Helm owned-resource filtering. They do not establish active application coverage.
- Type-only files, barrels, constants, command wrappers, bootstrap, fixed navigation catalogs, tiny badge/getter/scalar helpers and bounded form/request assembly. Measure substantive work through production callers.
- Browser development mocks, topology-spike generators/controllers, E2E scaffolding and test files. Their sanitized data shapes may inform fixtures.
- External I/O in kubeconfig/workspace files, keyring, filesystem/subprocess inspection, resource operations, HTTP/TLS/client creation and schema/tool execution. Benchmark only the identified in-memory transforms or prepared output parsing, and use live diagnostics for the enclosing operation.

## Benchmark contract

1. Name the user operation and the production function being measured. Call that function rather than copying its algorithm into a benchmark.
2. Reuse existing sanitized fixtures and workload sizes where they exercise the path. For new shapes, derive cardinality from real diagnostics or recorded fixtures. The fixture counts above are existing examples, not limits or recommended cluster sizes.
3. Prepare inputs outside measurement unless construction belongs to the operation. Distinguish cold construction, warm reuse, and updates. Include reset or cloning costs only when the production path pays them.
4. Fix timestamps, UUIDs and input ordering when they affect results. Simulate cache states with deterministic loaders and clocks; avoid sleeps and live network calls. Scheduling, contention and cancellation latency require complementary integration measurement.
5. Check output identity, counts, ordering, scope, health evidence, redaction and error behavior outside the timed closure. Keep trust-boundary validation and response bounds in the measured production path.
6. Record benchmark identity, fixture shape, revision, runtime/toolchain, features and measurement mode. Compare equal operations on identical inputs and inspect flamegraphs before choosing an optimization.
7. After improving the local stage, verify the affected application operation when claiming a user-visible improvement. CPU, allocations, retained heap, RSS, startup and painted frames are separate claims.

## CI and parallelism

[The workflow](../.github/workflows/codspeed.yml) enumerates every frontend `.bench.ts` file into an independent matrix job and runs Rust in parallel. The partition grows with the catalog rather than relying on a manually maintained job count. All jobs stay in one workflow for CodSpeed aggregation.

- [Vitest filtering](../vitest.config.ts) uses the same [file enumerator](../scripts/codspeed-benchmark-suites.mjs) as CI. `KUBECOVE_BENCHMARK_FILE` selects one known file. Unknown files, unknown legacy suites, and conflicting selectors fail. Legacy `topology` selects all topology benchmark files; `other` selects the complement. Regression checks prove exact-file jobs cover the catalog once and legacy partitions remain complete and disjoint.
- Rust CI explicitly builds/runs `backend_hot_paths`. Add cases to that target for automatic inclusion. A new target needs Cargo, workflow and local-script wiring.
- Existing hosted measurements above show frontend measurement dominates each former frontend job. File jobs remove sequential suite measurement from that job's critical path. Runner availability and repeated setup still affect total hosted time. [CodSpeed sharding](https://codspeed.io/docs/features/sharded-benchmarks)
- Shared `fixtures/` paths of any extension trigger both lanes. [Workflow regression tests](../scripts/codspeed-workflow-regression.test.mjs) cover fixture selection, actual file partitions, workflow matrix wiring and required-job completion. Missing comparison SHAs are fetched individually with depth one. Rust caches workspace artifacts and retains one target to avoid repeating Tauri linking across targets.
- The final `CodSpeed Performance Analysis` job verifies selected-job success. It defines no numerical regression threshold. External CodSpeed check configuration was not inspected.

## Existing commands

Run from the repository root with the existing dependencies and toolchains available:

```sh
bun run bench
bun run bench benchmarks/resource-table.bench.ts
bun run rust:bench
bun run rust:bench -- build_ownership_topology_500_apps
cargo run --release --manifest-path src-tauri/Cargo.toml --features bench-support --example cache_diagnostics
```

POSIX shell suite selection:

```sh
KUBECOVE_BENCHMARK_SUITE=topology bun run bench
KUBECOVE_BENCHMARK_SUITE=other bun run bench
KUBECOVE_BENCHMARK_FILE=benchmarks/yaml-editor.bench.ts bun run bench
```

On PowerShell, set `$env:KUBECOVE_BENCHMARK_SUITE` or `$env:KUBECOVE_BENCHMARK_FILE` for the invocation, then restore its previous value. The unfiltered command runs all frontend cases. `bun run typecheck:bench` checks benchmark fixtures and imports independently of the application build.

The existing CI Rust commands, from `src-tauri`, are:

```sh
cargo codspeed build --features bench-support --bench backend_hot_paths --locked
cargo codspeed run --bench backend_hot_paths
```

Ordinary local Vitest/Divan runs report native harness measurements. Bare `cargo codspeed run` outside an instrumented environment can check execution without producing CodSpeed measurements. [Divan integration](https://codspeed.io/docs/benchmarks/rust/divan)

Complementary commands:

```sh
bun run perf:frontend
bun run perf:resource-scope
bun run perf:startup --memory-fixture normal
bun run perf:startup --memory-fixture large
bun run perf:release
```

Native commands build/launch the application and write artifacts. Check their prerequisites and existing running instances before using them. See [development workflow](development-workflow.md), [frontend memory characterization](frontend-memory-retention.md), [backend cache diagnostics](backend-cache-diagnostics.md), and [release footprint measurements](release-footprint-variants.md).

## Frontend audit coverage

The inventories in this section record the original benchmark audit baseline. Later [legacy code and test cleanup](legacy-test-audit.md) removed proven uncalled paths; these historical counts and names are not a current filesystem manifest.

The parallel frontend audit inspected every source domain and all eight benchmark suites. The filesystem inventory contains 399 TypeScript/Svelte files: 166 non-test TS, 193 Svelte, 38 tests and two declarations. Of the 359 non-test implementation/module entries, 349 belong to production and ten to browser mocks or the topology harness. These include types and barrels, not just executable algorithms. The compact source manifest below expands to all 361 non-test/declaration paths with no missing or extra files against the filesystem inventory. Tests are counted separately and informed fixture contracts.

| Directory under `src/` | Non-test TS | Svelte | Tests | Declarations |
| --- | ---: | ---: | ---: | ---: |
| Root | 2 | 0 | 0 | 1 |
| `lib` | 55 | 0 | 13 | 0 |
| `app` | 7 | 18 | 3 | 0 |
| `features/workspaces` | 7 | 2 | 0 | 0 |
| `features/command-palette` | 3 | 0 | 0 | 0 |
| `features/rbac` | 7 | 4 | 0 | 0 |
| `features/live-sessions` | 7 | 3 | 5 | 0 |
| `features/resource-detail` | 19 | 15 | 8 | 0 |
| `features/resources` | 23 | 10 | 2 | 1 |
| `features/helm` | 3 | 2 | 0 | 0 |
| `features/argo` | 2 | 0 | 0 | 0 |
| `features/gitops` | 14 | 6 | 4 | 0 |
| `features/incidents` | 4 | 4 | 2 | 0 |
| `features/app-updates` | 4 | 0 | 0 | 0 |
| `components` | 6 | 13 | 1 | 0 |
| `components/ui/svelte` | 3 | 116 | 0 | 0 |
| Total | 166 | 193 | 38 | 2 |

No substantial local CPU candidate was found in app updating. Its work is network, installation, relaunch, small persistence and progress rendering. Shared Bits UI wrappers belong to application layout/interaction measurement; this catalog proposes no shared component changes.

### Complete non-test source manifest

Paths below are relative to `src/`. Within each list, names inherit the stated suffix.

#### Root and library

Root TS: `main`, `main-svelte`.

Production `lib` TS:

`app-version`, `argo-application-inspection`, `argo-application-inspection.svelte`, `argo-connection-policy`, `build-env`, `cancellation-types`, `diagnostics-report`, `diagnostics-types`, `diagnostics`, `error-redaction`, `finite-read-lifecycle`, `foreground-loading-svelte`, `foreground-loading`, `friendly-errors`, `gitops-ownership-evidence`, `gitops-resource`, `gitops-types`, `health-types`, `helm-types`, `kubeconfig-sources-query`, `memory-profile`, `path-state`, `query-retry`, `queryKeys`, `rbac-types`, `release-channel`, `resource-health`, `resource-identity`, `resource-metrics`, `resource-refresh`, `resource-watch`, `settings-store`, `settings`, `startup-marks`, `tauri-argo`, `tauri-args`, `tauri-diagnostics`, `tauri-inspection`, `tauri-runtime`, `tauri-streams`, `tauri`, `tree-nav`, `types`, `usage-metrics`, `utils`, `workspace-entry-points`, `workspace-model`, `workspaceReadContext`, `workspaces`, `yamlFormat`.

Browser-mock `lib` TS, excluded from production priorities:

`tauri-dev-mock-data`, `tauri-dev-mock-incidents`, `tauri-dev-mock-operations`, `tauri-dev-mock-rbac`, `tauri-dev-mocks`.

#### App

`app/svelte` TS:

`appUpdateStore`, `detailPanelFrameModel`, `diagnosticsSettingsModel`, `resourceVisuals`, `workspaceNavigation`, `workspaceShellModel`, `workspaceTransition`.

`app/svelte` Svelte:

`ActiveLiveSessionsButton`, `App`, `AppSurfaces`, `AppUsageFooter`, `ArgoConnectionSettings`, `CommandPalette`, `DetailPanelFrame`, `DiagnosticsSettings`, `ForegroundLoadingBar`, `KubeconfigSettings`, `Root`, `SettingsRow`, `SettingsSurface`, `SidebarTree`, `SidebarTreeNode`, `UpdateStatusButton`, `UpdatesSettings`, `WorkspaceShell`.

#### Features

| Directory under `features/` | TS names | Svelte names |
|---|---|---|
| `workspaces` | `index`, `query`, `workspace-sharing-parse`, `workspace-sharing-schema`, `workspace-sharing`, `workspaceLauncherModel`, `workspaceStore` | `WorkspaceLauncher`, `WorkspaceOverview` |
| `command-palette` | `entries`, `shortcut`, `store` | None |
| `rbac` | `cockpitModel`, `handoff`, `index`, `observedPermissions`, `reviewModel`, `risk`, `surfaceModel` | `RbacReviewBadge`, `RbacReviewPanel`, `RbacSurface`, `RbacView` |
| `live-sessions` | `helpers`, `index`, `liveSessionReadModel`, `podExecHelpers`, `podExecLifecycle`, `portForwardForms`, `portForwardLifecycle` | `LiveSessionsSurface`, `LiveSessionsView`, `SavedPortForwardForm` |
| `helm` | `helpers`, `index`, `surfaceState` | `HelmSurface`, `HelmView` |
| `argo` | `status-insights`, `status` | None |
| `gitops` | `argo-application-freshness`, `argo-operation-presentation`, `argo-operation-progress`, `argo-operation-tracker`, `argo-workspace-model`, `flux-kinds`, `gitops-nav`, `gitops-overview-helpers`, `index`, `surfaceModel`, `surfaceSelection`, `surfaceState`, `surfaceSummary`, `surfaceTooltips` | `ArgoApplicationDetails`, `ArgoApplicationWorkspaceHeader`, `ArgoOperationMonitor`, `ArgoOperationProgress`, `GitOpsSurface`, `GitOpsView` |
| `incidents` | `guidance`, `incident-actions`, `index`, `model` | `IncidentGuide`, `IncidentQueue`, `IncidentSurface`, `IncidentView` |
| `app-updates` | `api`, `index`, `store`, `types` | None |

Production `features/resources` TS:

`constants`, `grouping`, `helpers`, `index`, `operational-data`, `ownership-node-name`, `query`, `resourceBrowserModel`, `resourceBrowserReadSpecs`, `resourceTableModel`, `resourceTableTypes`, `table-state`, `topology-graph`, `topology-implementation`, `topology-layout`, `topology-standalone-groups`, `topology-status`, `topology-types`, `topology-view`, `topology`.

Production `features/resources` Svelte:

`NamespaceList`, `OwnershipMap`, `OwnershipMapViewport`, `OwnershipResourceNode`, `ResourceBrowser`, `ResourceBrowserTopBar`, `ResourceScopeSelector`, `StandaloneKindGroupNode`.

`features/resource-detail` TS:

`constants`, `deployment-revisions-model`, `eventsTabModel`, `execInput`, `execTerminal`, `helpers`, `incident-events`, `incident-signal-styles`, `incident-timeline-labels`, `incident-timeline`, `index`, `log-helpers`, `metadata-details`, `operations-model`, `resource-status`, `resourceDetailReadSpec`, `secretData`, `yamlApplyModel`, `yamlTabDiff`.

`features/resource-detail` Svelte:

`DeploymentRevisionsTab`, `DetailsTab`, `EventsTab`, `ExecTab`, `LogsTab`, `OperationsTab`, `PodExecLaunchForm`, `PodExecSessionList`, `PortForwardTab`, `ResourceDetailPanel`, `ResourceLogsPane`, `ResourceTimeline`, `ResourceYamlPane`, `SecretDataViewer`, `YamlTab`.

Development `features/resources/topology-spike`:

- TS: `benchmark-types`, `svelte-entry`, `synthetic-topology`.
- Svelte: `BenchmarkController`, `TopologyHarness`.
- Declaration/assets: `svelte.d.ts`, `svelte.html`, `topology-spike.css`.

#### Components

`components` TS:

`copy-text`, `sidebar-tree-helpers`, `status-badge-styles`, `timestamp-format`, `yamlCodeViewerDiagnostics`, `yamlCodeViewerExtensions`.

`components` Svelte:

`ClusterSelector`, `CopyToast`, `CopyableText`, `DeferredSurface`, `FriendlyError`, `HealthAssessmentBadge`, `OperationScopeCard`, `ResourceLiveStatusMenu`, `SimpleTable`, `StatGrid`, `SurfaceFrame`, `TimestampText`, `YamlCodeEditor`.

All 119 entries in `components/ui/svelte` were inspected. The following is a compact exact filename inventory: `Prefix{,Suffix}.svelte` includes the base filename and each listed suffixed filename.

- `Alert{,Action,Description,Title}.svelte`
- `Card{,Action,Content,Description,Footer,Header,Title}.svelte`
- `Command{,Dialog,Empty,Group,Input,Item,List,Separator,Shortcut}.svelte`
- `Dialog{,Close,Content,Description,Header,Overlay,Portal,Title,Trigger}.svelte`
- `Empty{,Content,Description,Header,Media,Title}.svelte`
- `Field{,Content,Description,Error,Group,Label,Legend,Separator,Set,Title}.svelte`
- `InputGroup{,Addon,Button,Input,Text,Textarea}.svelte`
- `Popover{,Content,Trigger}.svelte`
- `Resizable{,Handle,Panel}.svelte`
- `Select{,Content,Group,Item,Label,ScrollDownButton,ScrollUpButton,Separator,Trigger,Value}.svelte`
- `Sheet{,Close,Content,Description,Footer,Header,Title,Trigger}.svelte`
- `Sidebar{,Content,Footer,Group,GroupContent,GroupLabel,Header,Inset,Menu,MenuButton,MenuItem,Provider,Separator}.svelte`
- `Table{,Body,Caption,Cell,Footer,Head,Header,Row}.svelte`
- `Tabs{,Content,List,Trigger}.svelte`
- `Tooltip{,Content,Provider,Trigger}.svelte`
- Other Svelte: `Badge`, `Button`, `Checkbox`, `Input`, `Label`, `ScrollArea`, `SegmentedControl`, `Separator`, `Skeleton`, `Spinner`, `Switch`, `Textarea`.
- TS: `classes`, `index`, `types`.

The remaining declarations are root `bun-env.d.ts` and the topology-spike declaration already listed. The 38 `.test.ts` files are accounted for in the scope table and excluded from runtime candidate counts.

## Rust audit coverage

Rust source link destinations follow the later crate relocation; audit paths and line-number evidence retain their recorded baseline.

The Rust audit read and classified all 133 files under `src-tauri/src`: 118 non-test files and 15 dedicated test files. The inventory below was compared with the complete filesystem listing and has no missing or extra paths. `C` identifies candidate work above, `D` identifies live diagnostics, and `P` identifies plumbing or declarations exercised through callers. Inline test code supplied fixture/correctness evidence.

```text
Root
  lib.rs P/D
  main.rs P
  e2e.rs P/D, feature-gated setup excluded from production benches

commands/
  bench_support.rs P, existing benchmark bridges and fixtures
  builtin_kinds.rs P, fixed builtin registry
  cancellation.rs C/D
  contexts.rs D/P
  diagnostics.rs C/P
  discovery.rs C/D
  events.rs C/D
  gitops_crd.rs C/D
  helpers.rs P
  incidents.rs C/D
  kubeconfig.rs C/D
  kubeconfig_clients.rs D
  live_sessions.rs P/D
  live_store.rs C
  live_store_diagnostics.rs C
  live_store_refresh.rs C
  metrics.rs C/D
  mod.rs P
  namespaces.rs C/D
  operations.rs D/P
  pod_exec.rs P/D
  rbac.rs C/D
  rbac_inventory.rs D/P
  rbac_review.rs D/P
  rbac_risk.rs C
  resources.rs P
  sessions.rs P/D
  streams.rs P/D
  usage.rs C/D
  usage_webview.rs C
  workspace_files.rs D/P

commands/helpers/
  client_cache.rs C/D
  client_cancellation.rs D/P
  health.rs C
  metadata.rs C
  serialization.rs C
  time.rs C/P, controlled-time companion coverage
  validation.rs P, preserve trust-boundary checks
  metadata/ownership.rs C
  metadata/flux_inventory.rs C/D

commands/resources/
  apply.rs P/D
  details.rs P/D
  dynamic.rs C/D
  ingress_status.rs C, through summary fixtures
  kubeconform.rs C/D
  revisions.rs C/D
  scope.rs C/D
  summary.rs D/P
  summary_cluster.rs C/D
  summary_core.rs C/D
  summary_workloads.rs C/D
  topology.rs C/D
  topology_collection.rs C/D
  topology_dynamic.rs C/D
  topology_network.rs C
  yaml.rs C/D
  apply/execution.rs D/P, shared serialization covered through callers
  apply/linting.rs C/D
  apply/validation.rs C
  details/cluster.rs C/D
  details/core.rs C/D
  details/workloads.rs C/D

commands/helm/
  manifest.rs C, existing direct coverage
  mod.rs P
  reconciliation.rs C/D
  redaction.rs P, include in complete detail shaping
  storage.rs C/D
  time.rs P, include in record projection
  values.rs C

commands/argo/
  applications.rs C/D
  appprojects.rs C/D
  appsets.rs C/D
  comparison.rs C/D
  connected.rs C/D
  mod.rs P/D
  operations.rs C/D
  scope.rs P/D
  session.rs D/P
  transport.rs C/D
  tunnel.rs D/P

commands/flux/
  mod.rs C/D

commands/incidents/
  health.rs C
  restarts.rs C/D

commands/pod_exec/
  registry.rs C/D
  runner.rs C/D
  validation.rs P

commands/sessions/
  registry.rs C/D
  runner.rs D/P
  service.rs C/D
  target.rs P/D

commands/streams/
  aggregate_logs.rs C/D
  kinds.rs P
  logs.rs C/D
  registry.rs C/D
  watch.rs C/D

models/
  argo.rs C/P, ingress normalization plus DTOs
  cancellation.rs P
  cluster.rs P
  diagnostics.rs P
  discovery.rs P
  error.rs C/P
  events.rs P
  flux.rs P
  health.rs C
  helm.rs P
  incidents.rs P
  metrics.rs P
  mod.rs P
  namespace.rs P
  operations.rs P
  rbac.rs P
  resource.rs P, exercised by transformations/payload serialization
  sessions.rs P
  streams.rs P
  usage.rs P
```

The 15 dedicated test files are excluded from measured production work and retained as fixture/correctness evidence:

```text
commands/helpers/client_cache_tests.rs
commands/kubeconfig_tests.rs
commands/live_store_tests.rs
commands/live_store_diagnostics_tests.rs
commands/metrics_tests.rs
commands/rbac_tests.rs
commands/resources/apply_client_tests.rs
commands/resources/builtin_kind_read_tests.rs
commands/resources/topology_tests.rs
commands/helm/storage_tests.rs
commands/helm/reconciliation_tests.rs
commands/argo/operations_execution_tests.rs
commands/pod_exec/tests.rs
commands/sessions/tests.rs
commands/streams/watch_renewal_tests.rs
```

Inline `cfg(test)` code was likewise treated as correctness evidence. Dialog/file export, startup registration, fixed allowlists, command wrappers, tiny getters, and one-target guarded-operation validation do not warrant separate CodSpeed targets from the inspected workloads. Workspace import’s bounded UTF-8 conversion is locally measurable, but its dialog/filesystem path and infrequent trigger make it low priority.
