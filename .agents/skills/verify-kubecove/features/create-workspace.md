# Create a workspace

A workspace saves a cluster context, a namespace scope, and resource kinds under a name. The user creates one from the launcher, lands on its overview, and finds it again in the saved workspace list after a reload.

## Sub-features

- `create-defaults` creates a workspace for the default context with all namespaces.
- `create-scoped` limits the workspace to chosen namespaces.
- `create-save` names the workspace and opens its overview.
- `create-list` shows the saved workspace in the Workspaces list with its scope.
- `create-persist` keeps the saved workspace after a page reload.

## How to get to it (user POV)

- Open the app. With no saved workspaces, the launcher at `/#/workspaces` shows the `New workspace` form.
- From inside a workspace, choose `Open workspaces` in the top bar to return to the list and form.

## Driving it with kubecove-verify

Preconditions:

- A run from `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts launch` is ready, and `doctor` reports `"healthy": true`.
- No workspace is saved yet. A fresh run has none; otherwise run `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser reset`.

- **Before state.** Capture the empty launcher. Run `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser snapshot --role main --path create-workspace/before.aria.txt`. The snapshot shows `0 saved`, the `Name` textbox with placeholder `mock-dev`, the `Context` combobox set to `mock-dev`, and unchecked namespace checkboxes.
- **Scope namespaces.** Check one namespace. Run `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser click --role checkbox --name payments --exact`. The `payments` checkbox is checked.
- **Name it.** Type the name. Run `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser fill --role textbox --name Name --exact --value "Verify Lab"`.
- **Save.** Choose `Create workspace`. Run `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser click --role button --name "Create workspace"` and `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser wait --role heading --name "Verify Lab"`. The URL becomes `/#/workspace/<id>/overview`, and the overview reads `mock-dev`, `1 namespaces`, and a `Resource Health` total.
- **Saved list.** Choose `Open workspaces`. Run `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser click --role button --name "Open workspaces"` and `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser snapshot --role main --path create-workspace/list.aria.txt`. The snapshot shows `1 saved`, `Verify Lab mock-dev / payments`, and buttons `Open Verify Lab`, `Edit Verify Lab`, and `Delete Verify Lab`.
- **Persist.** Reload and reopen. Run `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser reload`, `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser click --role button --name "Open Verify Lab"`, and `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser wait --role heading --name "Verify Lab"`. The same overview opens.
- **Defaults variant.** For `create-defaults`, skip the namespace and name steps and choose `Create workspace` directly. The overview heading is `mock-dev` and reads `All namespaces`.
- **Proof.** Run `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser screenshot --path create-workspace/overview.png`. The screenshot shows the `Verify Lab` overview with `mock-dev`.

## Gotchas

- `Create workspace` is enabled before a name is typed; an empty name saves the context name, so the `create-defaults` overview heading is `mock-dev`.
- The `Name` textbox matches several labels without `--exact`.
- `goto` to a path that differs only in its `#` fragment does not navigate. Use the in-app buttons or `reload`.
- Saved workspaces live in this run's Chrome profile. `reset` and a new run both start empty.
- The `mock-dev` checkbox under `Cluster group` is checked and disabled; it marks the selected context, not a choice.
