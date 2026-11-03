# KubeCove verification map

This directory is the maintained source for verifying KubeCove's user-facing behavior in browser dev mode. Read this index before driving the app, then use the matching feature file as the recipe.

## Baseline preconditions

- Run commands from the repository root after `bun install`.
- Start a run with `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts launch`. It prints `"ready": true` once the launcher shows the `mock-dev` context.
- Run `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts doctor` and require `"healthy": true`.
- Data comes from the typed browser mocks in `src/lib/tauri-dev-mock-data.ts`. The launcher offers the contexts `mock-dev`, `admin@solid-k8s`, and `docker-desktop`, and namespaces such as `payments`, `operations`, and `opensearch`.
- Most journeys start inside a workspace. Reach one with the `create-save` steps in [Create a workspace](./create-workspace.md).
- Never drive a KubeCove window or Vite server that this run did not start. The helper only drives runs with a state file under `.e2e/verify/`.

## Driving conventions

- Commands spell out `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts` in full, because shell variables do not persist between agent tool calls.
- Later commands find the run on their own while exactly one run exists. Otherwise add `--run <id>`.
- Treat every command as literal. Keep quoted names and flags unchanged.
- Prefer `--role` with `--name` over `--selector`. Add `--exact` when one accessible name is a prefix of another, such as `Resources` and `Resources Overview`.
- The default viewport is 1440 x 900, where the sidebar tree is visible. At 768 px and 375 px, open the sidebar with `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser click --role button --name "Open workspace navigation"`.
- Start a journey from a fresh run, or run `.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser reset` to clear saved workspaces and settings in this run's profile.

## Proof and skip reporting

- Capture the user action and the resulting state: snapshot before and after the step under test.
- UI proof includes an ARIA snapshot (`browser snapshot`) and a screenshot (`browser screenshot`) in the run's evidence directory.
- Mutation proof includes a second, read-only view of the changed value, such as the table row or a reopened panel.
- Mock mutations change only in-memory mock state. Report them as UI-path proof, never as a Kubernetes write.
- Record the feature ID and entry point with every artifact; `actions.jsonl` logs each command.
- Report an unreachable path with the attempted command, its error, and the unmet precondition.
- Do not report a skipped entry point as verified through a different path.

## Feature entry contract

Each feature file starts with an H1 title and one paragraph describing the user-visible behavior. It then uses exactly four H2 sections in this order.

1. `Sub-features` lists short IDs with one line for each behavior.
2. `How to get to it (user POV)` lists every user entry point.
3. `Driving it with kubecove-verify` starts with `Preconditions:` and uses labeled bullets that pair each user action with an exact command and observable result.
4. `Gotchas` lists traps that can waste or invalidate a verification run.

Keep implementation details out of the map. Name only user paths, stable handles, required state, commands, and observable proof.

## Features

- [Create a workspace](./create-workspace.md) covers the launcher form, context and namespace choice, the saved workspace list, and persistence across reload.
- [Browse resources](./browse-resources.md) covers the Resources view, search, kind and namespace scope, and refresh.
- [Inspect a resource](./resource-details.md) covers the details panel, its tabs, pinning, and closing.
- [Scale a Deployment](./resource-actions.md) covers the guarded Actions tab: edit, server review, apply, and the second-view check.
- [Triage incidents](./incidents.md) covers the Incident Cockpit, its signal filters, and guided investigation.
