# Built-in resource summary projection

Contract: one object-to-summary projection per built-in kind feeds list rows,
`get_resource_details`, and ownership and network-flow topology. Details and
topology add only their own extras. List rows keep their previous serialized
output, and the health assessment keeps the [ADR 0018](decisions/0018-health-assessment-contract.md)
contract.

The projection is the `BuiltinSummary` trait in
[builtin_summary.rs](../src-tauri/crates/inspection/src/commands/resources/builtin_summary.rs).
Each kind implements it next to its list fetcher. The parity test in
[summary_parity_tests.rs](../src-tauri/crates/inspection/src/commands/resources/summary_parity_tests.rs)
serves one object through a mock Kubernetes client to the real list, details, and
topology entry points and compares the three summaries for every listed built-in kind.

## Disagreements before the change

Details built 14 summaries by hand. Topology formatted its own status text. Health
was parsed back out of the formatted status and readiness strings.

| Kind | Field | List | Details | Topology |
| --- | --- | --- | --- | --- |
| All except ReplicaSet and CRD | `apiVersion` | From the kind | Absent | From the kind |
| Pod | `status` with an empty phase | Absent, not evaluated | Empty string, health unknown | Absent |
| Deployment | `status` | `Available: N` when any count is nonzero | Absent | `Available: N` always |
| DaemonSet | `status` | Absent | Absent | `Available: N` |
| Ingress | `status`, `ready`, health | `Ready`/`true` or `Pending`/`false` | Same as list | Absent, not evaluated |
| Service | `status`, health | Absent, default assessment | Same as list | Network flow: Service type, health unknown |
| PersistentVolumeClaim | `status`, health | Absent, default assessment | Phase, assessed | Absent |
| Node, PersistentVolume, StorageClass | Owner, Helm, Argo CD and GitOps ownership | From metadata | Never set | From metadata |
| CustomResourceDefinition | `group`, `version`, `plural`, `namespaced`, `status`, health | Set | Set | Metadata only: none of them |
| All | `name`, `namespace` | Object metadata | Request arguments | Object metadata |

## Rule per field

The list rule wins unless noted. List rows feed the incident cockpit, and their
status and health are what users already see in tables.

- Identity comes from the object's type and metadata. Only CRDs add `group`,
  `version`, `plural`, and `namespaced`, because they describe their own API.
- Ownership evidence comes from metadata for every kind. `argoApp` is derived
  from the GitOps owner. `gitOpsOwnershipPartial` starts false; Flux inventory
  enrichment happens after projection.
- Age uses one clock reading per list, details read, or topology collection.
- Status, readiness, and restarts per kind:
  - Pod: non-empty phase, Ready condition status, checked restart sum.
  - Deployment: `Available: N` when available, ready, or desired is nonzero; `ready/desired`.
  - ReplicaSet: `Available: N`; `ready/replicas`.
  - StatefulSet and DaemonSet: readiness only.
  - Job: `Failed`, `Complete`, `Active`, or `Pending`; `succeeded/completions`, defaulting to one completion.
  - CronJob: `N active` when runs are active.
  - Ingress: `Ready`/`true` with a load-balancer address, otherwise `Pending`/`false`.
  - Node: Ready condition status; the count of `Ready=True` conditions over one.
  - PersistentVolume: phase.
  - CustomResourceDefinition: the `Established` condition, otherwise the first condition, as `Type: Status`.
  - Service, ConfigMap, Secret, PersistentVolumeClaim, StorageClass: none; the default assessment.

## Differences kept

1. Topology lists CRDs as metadata only, because CRD schemas are large. CRD nodes
   carry identity and ownership but no status or health.
2. Network flow labels Service nodes with their Service type through the node's own
   `status`. The node's summary keeps the shared, status-free Service summary.
3. Flux inventory ownership needs Kustomization and HelmRelease inventories, not the
   object itself. Scope lists and topology add it; details and `list_resources` do not.
4. Topology does not collect Nodes or PersistentVolumes.
5. PersistentVolumeClaims keep the list rule, so details no longer show the phase.
   Adopting the phase everywhere would label every bound claim's health unknown,
   because no rule classifies `Bound`. It would also turn intentionally unbound
   `Pending` claims into incident items. A typed phase rule for claims and volumes
   needs an owner decision.

## Visible changes

List rows are unchanged.

- Details: built-in kinds show their API version. Deployments show
  `Available: N`. Pods with an empty phase show no status. Nodes,
  PersistentVolumes, and StorageClasses show owner, Helm, and Argo CD or Flux
  ownership. PersistentVolumeClaims no longer show their phase, and their
  assessment becomes not evaluated.
- Topology: Ingress nodes show `Ready` or `Pending` with health. DaemonSet nodes
  no longer show `Available: N` and fall back to their health text. Deployments
  with all counts at zero no longer show `Available: 0`. CRD node summaries gain
  their API identity. Network-flow Service summaries keep the default assessment;
  the node label is unchanged.

## Health derivation

`kubernetes_health` classifies a phase the API reports, such as a Pod phase or a
Node Ready condition status, with typed readiness: absent, a Ready condition
status, or ready and desired counts. Workloads pass their counts directly. Jobs,
Ingresses, CronJobs, and CRDs choose their health together with their status text.
Nothing parses formatted `ready/desired`, `Available: N`, or keyword strings.

Evidence keeps the ADR 0018 shape: Kubernetes evidence with the displayed status and
readiness as raw values, and restart counts as non-current evidence.

A temporary test compared the complete serialized assessment from typed rules with
the previous string classifier for 3,947 generated objects across the 11 assessed
kinds. The inputs varied phases, condition statuses, case and whitespace, and
missing, zero, and negative counts. Every assessment matched. Only non-conforming
objects can differ: a Ready condition status shaped like `1/2` no longer parses as
counts, and a CRD whose selected condition is `Ready: True` is now unknown instead
of healthy.

## Measurements

Native Linux release medians, before at `05bb82b` and after this change. The two
benchmark binaries ran alternately for eight rounds, four in each order. Other
builds were running on the host; load averages were between 4 and 10.

| Benchmark | Before | After |
| --- | ---: | ---: |
| `typed_resource_summaries_10000` | 23.63 ms | 24.99 ms |
| `pod_list_decode_projected_1000` | 2.834 ms | 2.804 ms |
| `ownership_topology_production_mapping_500_apps` | 11.62 ms | 11.35 ms |
| `deployment_details_complete_inspection_40_containers` | 137.1 µs | 147.5 µs |
| `configmap_details_complete_inspection_40_keys` | 12.43 µs | 13.11 µs |
| `crd_details_complete_inspection` | 5.832 µs | 5.699 µs |
| `secret_details_discarded_yaml_redaction_and_dto` | 34.05 µs | 34.00 µs |
| `dynamic_details_complete_inspection` (unchanged code) | 24.62 µs | 25.39 µs |

The unchanged dynamic details control moved 3% in the same direction. Deployment and
ConfigMap details are dominated by YAML emission, which this change does not touch.
User-space instruction counts per iteration, from 5- and 25-iteration runs, stayed
within 0.4% for every changed case except ownership topology:

| Benchmark | Instructions before | Instructions after |
| --- | ---: | ---: |
| `typed_resource_summaries_10000` | 362.24 M | 362.54 M |
| `pod_list_decode_projected_1000` | 70.39 M | 70.26 M |
| `ownership_topology_production_mapping_500_apps` | 178.20 M | 173.84 M |
| `deployment_details_complete_inspection_40_containers` | 3.500 M | 3.488 M |
| `configmap_details_complete_inspection_40_keys` | 329.5 K | 328.8 K |

These counts include untimed input clones and output drops. For
`typed_resource_summaries_10000`, user-space cycles per iteration over six
alternating samples had medians of 293.4 M before and 294.2 M after, inside the
spread of either side. No added work is measurable; the wall-clock shifts are
within binary-layout and host-load noise. The ownership topology harness previously
built two summaries per typed workload object; it now calls the production input
builder once. CodSpeed instrumented runs were not performed locally.

```sh
cargo bench --manifest-path src-tauri/Cargo.toml --features bench-support --bench backend_hot_paths -- 'typed_resource_summaries_10000|pod_list_decode_projected_1000|ownership_topology_production_mapping_500_apps|details_complete_inspection|secret_details_discarded'
```
