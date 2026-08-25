# Incidents

**Incident Cockpit** gathers active warning, degraded, restart, and related workload signals from the workspace's configured namespace scope. It is a read-only investigation view, not an alerting system or automated root-cause engine.

## Investigate a signal

1. Open **Incidents** for the intended workspace and context.
2. Select **All**, **Degraded**, **Needs attention**, **Restart evidence**, or **Warnings**.
3. Select a signal and verify its context, namespace, kind, and object name.
4. Review the displayed status, conditions, events, and container evidence.
5. For Pods, inspect ownership topology to follow the signal toward its controller.
6. Select **Open resources** to carry the relevant namespace, search, and health filter into the Resource browser.
7. Refresh the source object before deciding whether a change is still needed.

Signals are assembled from the Kubernetes evidence KubeCove can read. A warning can be stale, transient, or secondary to another object. Correlate timestamps and ownership before naming a cause.

## Read summary counts carefully

| Summary | What it groups |
| --- | --- |
| **All** | Every active signal found in loaded scope. |
| **Degraded** | Available evidence showing degraded state. |
| **Needs attention** | Incomplete or warning evidence worth inspection. |
| **Restart evidence** | Container restart-related signals. |
| **Warnings** | Warning events or conditions found in available data. |

Counts can change as objects, conditions, and events update. Kubernetes events also expire. The cockpit is therefore an investigation starting point, not a durable incident record.

## Empty, partial, and error states

- **No active incident signals** and **Scope looks clean.** mean no signals were found in the loaded workspace scope. They do not prove the entire cluster is healthy.
- **No matching incident signals** means the selected severity filter hid other active results. Change the filter.
- **Partial incident data** means at least one evidence request failed or returned incomplete data. Treat counts and timelines as incomplete.
- **Incidents unavailable** means the primary incident request failed. Check context availability, connectivity, namespace scope, and read permissions.
- Selected-resource details or topology can fail independently even when the incident list loaded.

## Move from evidence to action

First identify the owning workload or controller. Then choose the least invasive recovery path:

- update the GitOps source or Helm workflow for desired-state problems;
- use the owning controller rather than repeatedly changing a generated Pod;
- use a supported guarded operation only after previewing its exact effect;
- capture sanitized timestamps and messages before transient events disappear.

Do not include Secret values, tokens, kubeconfig data, private repository URLs, or sensitive diagnostics in an incident report. See [Safety, data handling, and architecture](https://github.com/Timpan4/kubecove/wiki/Safety-Data-and-Architecture).

Next: [Resource details](https://github.com/Timpan4/kubecove/wiki/Resource-Details) and [Troubleshooting](https://github.com/Timpan4/kubecove/wiki/Troubleshooting).
