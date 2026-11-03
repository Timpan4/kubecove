# Scale a Deployment

The `Actions` tab offers guarded changes to the selected resource (ADR 0004, ADR 0024). The user edits the desired replica count, checks the change against a server dry run, applies it from the review dialog, and sees the new count in the table.

## Sub-features

- `actions-open` shows the guarded actions for the selected Deployment.
- `actions-edit-replicas` stages a new replica count as a pending edit.
- `actions-review` shows the server dry-run diff in a review dialog before anything changes.
- `actions-apply` applies the reviewed change and reports the result.
- `actions-second-view` shows the new count in the Resources table and in a reopened panel.

## How to get to it (user POV)

- Open a Deployment's details panel from the Resources table and choose the `Actions` tab.
- Open the Deployment from the Incident Cockpit with `Open in Resources`, then choose `Actions`.

## Driving it with kubecove-verify

Preconditions:

- A ready run with a `mock-dev` workspace for all namespaces, on the Resources view with the search `payments-api`. Use [Create a workspace](./create-workspace.md) and the first two steps of [Browse resources](./browse-resources.md).
- `Skip change review` is off in Settings, which is the default in a fresh run.

- **Open the Deployment.** Choose the Deployment row's name. Run `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser click --within 'tr:has(td:text-is("Deployment"))' --role button --name "Open resource payments-api" --exact` and `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser click --role tab --name Actions`.
- **Before state.** Capture the actions. Run `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser snapshot --role tabpanel --name Actions --path resource-actions/before.aria.txt`. The snapshot shows `Desired 2`, a `Replicas` article with `2 desired`, and `Edit`, `Pause`, `Restart`, `Roll back`, and `Delete` buttons.
- **Edit replicas.** Choose `Edit` on `Replicas` and enter `3`. Run `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser click --within 'article:has-text("Replicas")' --role button --name Edit` and `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser fill --label "Desired replicas" --value 3`. The `Replicas` article shows `Pending`.
- **Review.** Choose `Check changes`. Run `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser click --role button --name "Check changes"` and `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser snapshot --role dialog --path resource-actions/review.aria.txt`. The dialog `Review edit for payments-api` names `edit Deployment payments/payments-api in context mock-dev`, shows `- replicas: 2` and `+ replicas: 3` in its `API server result` diff, and has `Cancel` and `Apply`.
- **Apply.** Choose `Apply`. Run `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser click --within '[role=dialog]' --role button --name Apply` and `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser wait --text "Simulated edit of Deployment payments/payments-api"`. The dialog closes, the status reads `Simulated edit of Deployment payments/payments-api in context mock-dev in browser mock mode.`, and the tab shows `Desired 3`.
- **After state.** Run `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser snapshot --role tabpanel --name Actions --path resource-actions/after.aria.txt`. The `Replicas` article shows `3 desired`.
- **Second view.** Close the panel and read the table row. Run `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser click --role button --name "Close resource details"` and `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser snapshot --selector 'tr:has(td:text-is("Deployment")):has(button[aria-label="Open resource payments-api"])' --path resource-actions/row.aria.txt`. The `Readiness not reported` cell reads `3/3`.
- **Reopen.** Open the Deployment again and choose `Actions`, as in the first step. Run `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser text --role tabpanel --name Actions --path resource-actions/reopened.txt`. The text starts with `Desired 3`.
- **Proof.** Run `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser screenshot --path resource-actions/after.png`. The screenshot shows the reopened Deployment panel with `Desired 3`.

## Gotchas

- Mock mode simulates the dry run and the apply. The status says `in browser mock mode`; this proves the guarded UI path, not a Kubernetes write. Prove a real scale on the Kind lab.
- `Open resource payments-api` matches the Deployment, Service, and Ingress rows. Always scope to the Deployment row.
- The table's Ready cell has the accessible name `Readiness not reported` in mock data; assert on its text, `3/3`.
- With `Skip change review` on (Settings, `Advanced`), `Check changes` applies inline without a dialog. Check that setting first when the review dialog does not appear.
- Mock state lives in the page. `reload` and `reset` restore `Desired 2`.
