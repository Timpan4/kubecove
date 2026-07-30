# Settings, updates, and diagnostics

Open **Settings** from app navigation. Settings are global preferences, not workspace-specific controls. Argo CD appears as a category for discovery, but connect, reconnect, disconnect, and forget actions remain in Application details.

## General

| Control | Default | Effect and risk |
| --- | --- | --- |
| Exact timestamps | Off | Adds absolute times beside relative ages. |
| Timestamp timezone | Local | Changes exact timestamps and tooltips to local time or UTC. |
| CPU and memory footer | Off | Shows app-process usage only. |
| Ownership map by default | On | Opens ownership map in resource views. |
| Redact secrets | On | Keeps Secret values hidden by default. Connected Argo Secret values remain masked. |
| Full map during selection | Off | Keeps unrelated ownership branches visible; large namespaces can render more slowly. |
| Unavailable GitOps providers | Off | Shows disabled Argo CD and Flux groups when their CRDs are absent. |
| Custom Resources | On | Loads Custom Resources after native resources are visible. |

## Live sessions

| Control | Default | Effect and risk |
| --- | --- | --- |
| Auto-start saved port forwards | Off | Starts saved Service port-forward presets when restoring a workspace. Verify target and local port before enabling. |
| Keep live sessions across workspace switches | Off | Leaves port-forward and Pod exec sessions running after leaving their scope. Stop sessions explicitly when no longer needed. |

## YAML

| Control | Default | Effect and risk |
| --- | --- | --- |
| YAML shape | Kubectl view | Select Kubectl view or Apply clean output. |
| YAML encoding | YAML | Select YAML or KYAML in YAML panels. |
| YAML diff appearance | Clean | Select clean or Git-style rendering for selected-resource dry-run diffs. |
| YAML error lens | On | Shows editor diagnostics below YAML lines. |
| Allow YAML force-conflicts | On | Allows guarded YAML operations to take server-side field ownership. Review dry-run diff and confirmation target before force-applying. |

## Kubeconfig

Kubeconfig sources default to `KUBECONFIG`; when it is unset, standard default discovery applies. Source labels are on by default. Add files with file picker or pasted paths, then order them deliberately: sources are read in configured order. Full paths appear only in this Settings section.

Changing source, paths, or order changes contexts available to workspaces. Remove stale paths rather than leaving them to generate load warnings.

## Updates

Open **Updates**, select **Check**, then select **Install** only after reviewing available release information. Installation downloads updater artifact, then requires **Relaunch** to finish. Development builds disable update checks.

If a check fails, retry after confirming network access to GitHub release downloads. Do not replace application files manually while installer is running.

## Diagnostics

![Diagnostics latency report in Settings](https://raw.githubusercontent.com/Timpan4/kubecove/main/docs/assets/wiki/settings-diagnostics-report.png)

Diagnostics are off by default. Enable them to collect frontend and backend latency timings, render counters, and bounded trace events in memory. They are intended for local investigation; disable them when finished.

**Latency report** shows frontend timings, render counters, and backend timings. **Copy** creates identifier-redacted output by default. Leave **Include identifiers** unchecked unless recipient and destination are trusted. **Clear** removes frontend and backend trace data from memory.

For report contents, retention, and support sharing, see [Safety, data handling, and architecture](https://github.com/Timpan4/kubecove/wiki/Safety-Data-and-Architecture).
