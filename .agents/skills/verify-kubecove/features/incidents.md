# Triage incidents

The Incident Cockpit lists health signals across the workspace, grouped by GitOps ownership and sorted by severity. The user filters the queue, selects a signal to read its guided investigation, and opens the affected resource in Resources.

## Sub-features

- `incidents-open` opens the cockpit with signal counts by category.
- `incidents-filter` narrows the queue to one category, such as `Degraded`.
- `incidents-select` shows the guided investigation for a selected signal.
- `incidents-open-resource` opens the selected resource's details panel in Resources.

## How to get to it (user POV)

- Choose `Incidents` in the sidebar tree under `Workspace`.
- Choose `Incidents` under `Operations` on the workspace overview.
- Choose an `Incident shortcuts` button, such as `Degraded 3`, on the workspace overview.

## Driving it with kubecove-verify

Preconditions:

- A ready run with a `mock-dev` workspace for all namespaces. Use the `create-defaults` steps in [Create a workspace](./create-workspace.md).

- **Open.** Choose `Incidents` in the sidebar. Run `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser click --role treeitem --name Incidents --exact` and `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser wait --role heading --name "Incident Cockpit"`. The URL ends in `/incidents`. Buttons read `All 10`, `Degraded 3`, `Needs attention 5`, `Restart evidence 2`, and `Warnings 9`.
- **Filter.** Choose `Degraded 3`. Run `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser click --role button --name "Degraded 3" --exact` and `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser count --selector 'main main article'`. `Degraded 3` is pressed and the count is `3`.
- **Select.** Choose the evicted Pod. Run `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser click --role button --name "Pod/operations-worker-evicted"` and `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser snapshot --selector 'main main' --path incidents/investigation.aria.txt`. The signal is pressed, and the investigation shows the heading `worker terminated: ContainerStatusUnknown`, sections `Confirmed evidence`, `Still unknown`, `Next investigation steps`, and `Available guarded actions`, and the button `Open in Resources`.
- **Open the resource.** Choose `Open in Resources`. Run `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser click --role button --name "Open in Resources"` and `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser wait --role heading --name operations-worker-evicted --exact`. The URL ends in `/resources` and the details panel for `operations-worker-evicted` is open.
- **Overview entry.** From the workspace overview, choose `Incidents` under `Operations`. Run `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser click --role button --name Incidents --exact` and `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser wait --role heading --name "Incident Cockpit"`. The URL ends in `/incidents`.
- **Shortcut entry.** From the workspace overview, choose `Degraded 3` under `Incident shortcuts`. Run `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser click --role button --name "Degraded 3" --exact` and `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser wait --role heading --name "Incident Cockpit"`. The cockpit opens with `Degraded 3` pressed.
- **Proof.** Run `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser screenshot --path incidents/opened.png`. The screenshot shows the opened panel.

## Gotchas

- Mock incident data ignores the workspace's namespace scope: a workspace scoped to `payments` still lists signals from `operations` and `opensearch`. Prove namespace filtering of signals on the Kind lab.
- The overview's `Resource Health` counts cover only the workspace kinds, while the cockpit also counts kinds such as Argo CD `Application`. The overview's `Needs attention 4` and the cockpit's `Needs attention 5` are both expected for the default workspace.
- Signal buttons carry long accessible names. Match the leading `<Kind>/<name>` without `--exact`.
- The first signal is selected on open. Select another one before asserting on the investigation text.
