# Headless memory investigation and optimization

Measured on 2026-10-11 for the reported 0.12.0 memory issue. The implementation
removes full CRD schema decoding and duplicate response buffers from resource-kind discovery. It preserves
the Kubernetes endpoint, credentials, RBAC handling, cache policy, concurrency,
catalog fields, and version selection. The full browser image was deployed to
wall-e at the user's request on 2026-10-11. Initial production checks passed;
the user chose to exercise the app for the remaining interactive memory test.

## Live evidence

The supplied kubeconfig selected context `wall-e`. The reported pod was in
namespace `kubecove`.

| Measurement | Result |
| --- | --- |
| Server container, `kubectl top` | 809 MiB |
| Cloudflare Tunnel sidecar | 22 MiB |
| Server anonymous RSS, `/proc/1/status` | 823,788 KiB |
| Server file RSS | 7,124 KiB |
| Threads / allowed CPUs | 19 / 16 |
| CRD list response | 2,895,243 bytes, 63 CRDs |
| CRD schema nodes in that response | 20,413 |

The deployed server was an imported image, not the public release digest:
`docker.io/library/kubecove-server@sha256:9db27b30ccc9e0880a705b25e922ac83805bf7cc2797a705de96028ecd8b45fd`.
Its exact source revision was not established. The latest published release
was `app-v0.12.0`. The public image tested locally was
`ghcr.io/timpan4/kubecove@sha256:8691420f2b083cd907131541fa040dbed6b4d7725a2fbfb7aa7563b4cd98efda`,
labelled version 0.12.0 and revision `d714d0b1953d8d0a4e1b1a9c4ec0aa957ff917df`.

The live process had sixteen large anonymous mappings consistent with glibc
arenas. Thread stacks, file-backed assets, and the sidecar did not account for
the server's RSS. On macOS, a warmed baseline server had 75.4 MiB resident in
malloc regions but only 421 KiB of live malloc allocations after its session
closed. That measurement is specific to macOS, but also demonstrates the
difference between live cached data and retained allocator pages.

## Mechanism and change

Previously, `crd_resource_kinds` listed `CustomResourceDefinition` objects.
That deserialized every schema node, metadata field and status field, then
discarded them to produce a small catalog. Repeated cache refreshes caused
these large transient allocations on different Tokio workers. The live Linux
memory shape and local experiments support allocator retention as the dominant
cause, rather than a cache containing the schemas.

Discovery now decodes an internal projection containing only `group`, `names`,
`scope`, and each version's `name`, `served`, and `storage`. Serde skips the
unused fields without building their trees. The public `DiscoveredResourceKind`
model is unchanged. Storage preference, first-served fallback, sorting,
deduplication, optional short names, and scope interpretation are preserved.
Type metadata remains string-validated. Missing/null catalog fields retain
the generated Kubernetes types' defaults. Unused schema fields are parsed as
JSON but no longer validated against the full generated schema model.

The request still uses kube-rs's CRD resource URL, list request builder, client,
and HTTP error handling. `Client::request_stream` consumes HTTP chunks into one
buffer, then `String::from_utf8` reuses that buffer. This avoids retaining all
HTTP chunks while collecting and copying the complete response. The JSON body
still needs one complete buffer. UTF-8 and JSON failures remain serialization
errors, and body read failures retain their source chain for cancellation.
See the [kube-rs client API](https://docs.rs/kube/4.2.0/kube/client/struct.Client.html).

## Measurements

### Native macOS, real cluster, full browser UI build

| Stage | Baseline RSS, MiB | Optimized RSS, MiB |
| --- | ---: | ---: |
| Authenticated idle | 16.8 | 17.0 |
| Namespaces and CRD discovery | 85.5 | 29.6 |
| Eight resource-kind lists | 91.0 | 36.5 |
| Ownership topology | 93.7 | 46.0 |
| Network topology | 94.1 | 46.3 |

Live objects changed between these runs. These are workload measurements, not
exact topology-output comparisons. Both runs discovered 22 namespaces and 63
CRD kinds, 245 rows across eight lists, 292 ownership nodes and 76 network nodes.
The full and slim baseline builds had similar RSS, ruling out
embedded frontend assets as the main allocation source.

### Linux, isolated replay of the same CRD response

The replay uses an amd64 container under Rosetta in a local Podman VM. It
contains no cluster credentials. Sixteen workers reproduce the affected pod's
allowed CPU count. Each refresh really refetches the fixture. Sixteen context
aliases exercise concurrent discovery independently; that burst is a stress
case, not a claim about the user's normal number of contexts.

Baseline experiments established allocator sensitivity:

| Runtime | After 16 sequential refreshes, MiB RSS | After concurrent contexts, MiB RSS |
| --- | ---: | ---: |
| Default allocator, 16 workers | 188.3 | 483.3 |
| `MALLOC_ARENA_MAX=1`, 16 workers | 90.3 | 266.0 |
| Default allocator, 1 worker | 82.3 | 156.2 |

The catalog's shallow cache payload was 9,600 bytes for one context and 153,600
bytes for sixteen. These values exclude strings and container overhead; they
must not be treated as exact heap allocation totals.

Neither worker reduction nor an arena cap is part of the source change. GNU
describes arenas as a means to reduce allocation contention across threads;
reducing them can trade memory for contention. Removing allocations avoids
that tradeoff. See the [GNU allocator documentation](https://www.gnu.org/software/libc/manual/2.35/html_node/The-GNU-Allocator.html)
and [arena tunables](https://sourceware.org/glibc/manual/latest/html_node/Malloc-Tunable-Parameters.html).

The controlled comparison below uses the public 0.12.0 browser image and local
slim backend candidates. The native full/slim comparison above separates the
embedded frontend from the measured allocation source. All replay runs use
the same 2,895,243-byte response, sixteen workers and the default allocator.

| Stage | Public 0.12.0 RSS, MiB | Schema projection RSS, MiB | Final projection and body buffer RSS, MiB |
| --- | ---: | ---: | ---: |
| Authenticated idle | 23.5 | 22.2 | 22.4 |
| One discovery | 76.1 | 35.6 | 30.9 |
| Sixteen sequential refreshes | 193.3 | 63.0 | 43.1 |
| Sixteen contexts, concurrent | 462.2 | 118.3 | 76.7 |

The final change reduced retained RSS by 78% after sequential refreshes and
83% after the concurrent burst. All 63 returned catalog entries match the
baseline exactly, including every exposed field. The sixteen sequential
refreshes generated sixteen new CRD list requests; this was not a cache hit
comparison. The final local image ID is
`50d6654ebdadf41fa6ea1b180a3a9842982e0d314d46d8f02f2729951cb529e9`.

Discovery RPC latency in this replay fell from a 181 ms median to 120 ms for
the sixteen sequential refreshes. The cold request fell from 310 ms to 209 ms.
These timings include local transport and emulation; they are not native
production latency predictions.

The deterministic decoder benchmark measured a 1.572 ms median for the narrow
catalog decoder and 14.62 ms for the full generated CRD decoder on the same
synthetic input, about 9.3 times faster. Means were 2.212 ms and 15.75 ms.
The response-buffer change does not alter that decoder benchmark.

### Native wall-e deployment

The full candidate passed the same Linux replay before deployment: 39.7 MiB
after sixteen sequential refreshes and 80.2 MiB after concurrent discovery.
Its image configuration is `sha256:fdda3516fb5ced0c1c5fa77e751ae261c992caa3656b0a534c0ffe915942e193`.
The imported manifest is `sha256:f573919a2c6d91fb766e2ff2c02824094b298a5c99dc7fe628f7b97bd20f3891`.

Deployment revision 7 started pod `kubecove-6df76cc84b-dkjb8` at
2026-10-11T17:10:11Z. Both containers became ready with zero restarts. Only the
server image, command and arguments changed. The shell wrapper became a direct
binary invocation with `--quiet`, because the new runtime has no shell.
Authentication, TLS, storage, permissions, worker count, and the sidecar stayed
unchanged. The previous image remains available, and a guarded rollback patch
is saved under `e2e/artifacts/headless-memory/wall-e-rollback-patch.json`.

| Native process sample | RSS, MiB | Anonymous RSS, MiB |
| --- | ---: | ---: |
| Cold after rollout, 17:10:45 UTC | 17.3 | 2.4 |
| After OIDC sign-in, workspace listing and UI fetch, 17:12:18 UTC | 25.5 | 3.3 |
| Latest sample after user took over UI testing, 17:13:24 UTC | 60.8 | 36.6 |

The process retained nineteen threads and sixteen allowed CPUs. Arc completed
OIDC sign-in and rendered the existing in-cluster workspace and namespace
choices. The public endpoint served the embedded frontend and retained OIDC
mode. The exact workload between the latter two samples was not recorded.
These initial samples are not a warmed-workload comparison with the old
811 MiB process. Repeated production resource refreshes and a prolonged soak
remain unverified. Concurrent Arc automation interfered with further UI checks;
the user elected to exercise the deployed app directly.

## Reproduction and verification

Local evidence and temporary profiling drivers are under
`e2e/artifacts/headless-memory/`, which is ignored by Git. The captured fixture
stays local. Tokens and test TLS keys must not be committed.

The permanent CodSpeed benchmark generates synthetic input matching the
observed 63 CRDs and approximately 20,413 schema nodes, without embedding
cluster data. It compares the projection against full CRD decoding on exactly
the same input. The full reference only counts objects, so it excludes the
old catalog-construction cost and is a conservative speed comparison.

```sh
cargo test --locked --manifest-path src-tauri/Cargo.toml commands::discovery::tests --lib
cargo check --locked --manifest-path src-tauri/Cargo.toml
cargo clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
cargo bench --locked --manifest-path src-tauri/Cargo.toml --features bench-support --bench backend_hot_paths -- decode_
```

The authenticated optimized browser was checked against wall-e: workspace
creation, the 240-resource overview, and custom resource group discovery.
The final rebuilt server also rendered all 18 Argo Applications. The screenshot
is `e2e/artifacts/headless-memory/final-custom-resources.png`.
The local investigation accessed cluster resources only for reads. The later
authorized production test updated the KubeCove Deployment as described above.
The user's desktop app was preserved.

Existing backend unit tests do not compile with `--no-default-features` because
unrelated tests directly use Tauri APIs. The focused tests use the repository's
normal desktop feature set; headless production is checked through its own
release build and API replay.

Focused discovery tests (eight), existing error-classification tests (five),
Cargo check, strict all-target/all-feature
Clippy, formatting, and native/Linux release builds passed. The final body
collection change additionally preserves invalid UTF-8 classification.

Pi with `openai/gpt-6-luna` independently reviewed CRD compatibility and
session/stream retention. The projection source review reported no valid-response
behavior findings. An additional body-buffer review stalled on approval for a
read-only Git command and was cancelled. The parent checked the final client
and error paths directly against kube-rs source. One allocator research task
remained blocked by a browser approval and was cancelled; the parent supplied
authoritative-source research and Linux experiments.

## Other findings and limits

Channel history is bounded by message count, not bytes. Long log lines during
a disconnected session can retain substantial data. There was no evidence
that this condition caused the reported idle RSS, and changing replay history
would need an explicit UX contract. No speculative byte limit was introduced.

This investigation does not establish a universal RAM ceiling for arbitrary
clusters, workloads or sessions. It verifies a specific transient allocation
source and its counterfactual on identical discovery input. A native Linux
production soak remains necessary to measure the deployed workload over time.
The optimized full image is deployed on wall-e for that user-run test.
