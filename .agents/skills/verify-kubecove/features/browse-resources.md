# Browse resources

The Resources view lists the workspace's resources in one table, grouped by owner and kind, with health counts and a graph. The user searches it, narrows the kinds or namespaces, and refreshes it without losing the search.

## Sub-features

- `browse-open` opens the Resources view from the overview or the sidebar.
- `browse-search` filters rows by name, namespace, kind, owner, GitOps owner, or Helm release.
- `browse-kinds` adds or removes resource kinds from the table.
- `browse-namespaces` changes the namespace scope.
- `browse-refresh` refreshes with a cleared cache and keeps the search.

## How to get to it (user POV)

- Choose `Open resources` in the workspace overview header, or `Resources` under `Shortcuts`.
- Press `Ctrl K` and search views, namespaces, and resources.

## Driving it with kubecove-verify

Preconditions:

- A ready run with a workspace for `mock-dev` and all namespaces. Use the `create-defaults` steps in [Create a workspace](./create-workspace.md).

- **Open the view.** Choose `Open resources` in the overview header. Run `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser click --role button --name "Open resources" --exact`. The URL ends in `/resources`, and the region `Resource controls` shows `Namespace All` and `Kinds Pod, Deployment, ReplicaSet +9`.
- **Search.** Type a name. Run `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser fill --label "Search resources" --value payments-api` and `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser snapshot --role table --path browse-resources/search.aria.txt`. The table shows the Pod row `Open resource payments-api-7d9c9b7f8d-k2r9p` and a Deployment row `Open resource payments-api`. The count `Total 7` is a button in the `Resource controls` region, outside the table; `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser snapshot --role region --name "Resource controls" --path browse-resources/controls.aria.txt` shows it.
- **Remove a kind.** Open the kind editor and clear `Pod`. Run `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser click --role button --name "Edit resource kinds"`, `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser click --role checkbox --name Pod --exact`, and `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser press --key Escape`. Then run `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser count --role button --name "Open resource payments-api-7d9c9b7f8d-k2r9p"`. The count is `0`, and `Edit resource kinds` no longer lists `Pod`. Check `Pod` again the same way to restore it.
- **Refresh.** Open live options and refresh. Run `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser click --role button --name "Live updates and refresh options"`, `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser click --role button --name "Refresh (clear cache)"`, and `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser press --key Escape`. The popover shows the status `Live updates connected`. `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser count --text "Refresh failed:"` prints `0`, and `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser value --label "Search resources"` still prints `payments-api`.
- **Namespace scope.** Clear `payments` from the scope. Run `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser click --role button --name "Edit namespace scope"`, `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser click --role checkbox --name payments --exact`, and `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser press --key Escape`. `Edit namespace scope` reads `Namespace default, argocd +13`, and `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser count --role button --name "Open resource payments-api" --exact` prints `0`. Repeat the three commands to check `payments` again; the count returns to `3`.
- **Palette entry.** From a fresh workspace overview, press `Ctrl K` and search. Run `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser press --key Control+k`, `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser fill --within '[role=dialog]' --role combobox --value payments-api`, and `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser snapshot --role listbox --path browse-resources/palette.aria.txt`. The dialog `Search` lists options such as `payments-api mock-dev payments Deployment`. Choose it with `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser click --role option --name "payments-api mock-dev payments Deployment"`; the URL ends in `/resources` and the details panel for `payments-api` opens.
- **Proof.** Run `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser screenshot --path browse-resources/table.png`. The screenshot shows the search value and the filtered rows.

## Gotchas

- Three resources share the name `payments-api`: a Deployment, a Service, and an Ingress. Scope a row with `--within 'tr:has(td:text-is("Deployment"))'`.
- The kind editor and the live options open as popovers outside `main`. Snapshot the whole page, not `--role main`, to see them.
- Row buttons such as `Copy resource name: <name>`, the health button, and `Pin <Kind> <name>` must not open the details panel. Click `Open resource <name>` to open it.
- After the palette opened the Deployment and the search was refilled, the Pod row button `Open resource payments-api-7d9c9b7f8d-k2r9p` was not found. Run table steps before the palette entry, or from a fresh workspace.
- The namespace popover starts with every namespace checked; `Select all namespaces` is disabled until one is cleared.
- A workspace scoped to `payments` shows `Total 9` before any search; counts change with the scope.
