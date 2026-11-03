# Inspect a resource

The details panel opens beside the Resources table for one resource. It shows health evidence, metadata, events, logs, live-session tabs, guarded actions, and YAML, and the user can pin the resource or close the panel.

## Sub-features

- `details-open` opens the panel from a row's name button.
- `details-summary` shows health, readiness, restarts, the incident timeline, and metadata on the `Details` tab.
- `details-events` lists events with type filters on the `Events` tab.
- `details-yaml` shows the YAML view with shape and encoding choices on the `YAML` tab.
- `details-pin` pins the resource to the workspace overview.
- `details-close` closes the panel.

## How to get to it (user POV)

- Choose a resource name in the Resources table.
- Focus a row's name button and press `Enter`.
- Choose a resource in the `Ctrl K` search; see [Browse resources](./browse-resources.md).
- Choose `Open in Resources` on a signal in the Incident Cockpit; see [Triage incidents](./incidents.md).

## Driving it with kubecove-verify

Preconditions:

- A ready run with a `mock-dev` workspace for all namespaces, on the Resources view with the search `payments-api`. Use [Create a workspace](./create-workspace.md) and the first two steps of [Browse resources](./browse-resources.md).

- **Open.** Choose the Pod name. Run `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser click --role button --name "Open resource payments-api-7d9c9b7f8d-k2r9p"` and `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser wait --role button --name "Close resource details"`. The panel heading reads `payments-api-7d9c9b7f8d-k2r9p` with `Pod / payments`, and the tabs are `Details`, `Events`, `Logs`, `Exec`, `Forward`, `Actions`, and `YAML`.
- **Summary.** Capture the `Details` tab. Run `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser snapshot --role tabpanel --name Details --path resource-details/details.aria.txt`. The snapshot shows `Needs attention`, `Ready 1/1`, `Restarts 2`, a `Timeline` entry `Readiness probe failed`, and `Owned by Argo CD: shop`.
- **Events.** Choose `Events`. Run `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser click --role tab --name Events` and `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser snapshot --role tabpanel --name Events --path resource-details/events.aria.txt`. The snapshot shows `Total 2 events`, `Warnings 1`, the textbox `Filter resource events`, and a radio group with `All`, `Warnings`, and `Normal`.
- **YAML.** Choose `YAML`. Run `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser click --role tab --name YAML` and `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser snapshot --role tabpanel --name YAML --path resource-details/yaml.aria.txt`. The snapshot shows `mock-dev / payments / Pod / payments-api-7d9c9b7f8d-k2r9p`, the combobox `YAML shape` set to `Kubectl view`, the combobox `YAML encoding`, and the button `Edit YAML`.
- **Pin.** Choose the pin button. Run `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser click --role button --name "Pin Pod payments-api-7d9c9b7f8d-k2r9p" --within 'aside:has([aria-label="Close resource details"])'`. Both pin buttons change to `Unpin Pod payments-api-7d9c9b7f8d-k2r9p`: `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser count --role button --name "Unpin Pod payments-api-7d9c9b7f8d-k2r9p"` prints `2`, one in the panel and one in the table row.
- **Close.** Choose `Close resource details`. Run `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser click --role button --name "Close resource details"` and `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser wait --role button --name "Close resource details" --state hidden`. The table fills the view again.
- **Keyboard entry.** From the same search, press `Enter` on the row's name button. Run `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser press --role button --name "Open resource payments-api-7d9c9b7f8d-k2r9p" --key Enter` and `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser wait --role heading --name "payments-api-7d9c9b7f8d-k2r9p" --exact`. The panel opens.
- **Proof.** With the panel open, run `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser screenshot --path resource-details/panel.png`. The screenshot shows the table and the open panel with its heading.

## Gotchas

- The `Logs` tab in mock mode shows `No containers found` and `Container status has not loaded for this Pod yet.` Prove log streaming on the desktop app or the Kind lab.
- `Exec` and `Forward` are live-session tabs (ADR 0003, ADR 0005). This map does not cover them; a real session needs the desktop app and a cluster.
- The pin button appears both in the table row and in the panel. Scope it with `--within 'aside:has([aria-label="Close resource details"])'`. The panel is an `aside`; `[role=complementary]` does not match it.
- The YAML editor does not expose its full text through `text`; assert on the snapshot header and controls.
