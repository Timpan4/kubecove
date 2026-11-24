---
name: verify-kubecove
description: Drive KubeCove and capture evidence in the running app. Covers the Svelte workspace in headless Chrome with typed mocks, the headless server and its MCP endpoint, the GPUI desktop smoke, and the Kind lab. Use for frontend, workspace, resource-browser, detail-panel, guarded-action, incident, server, and MCP verification.
---

# Verify KubeCove

The headless server serves the Svelte browser client. In Vite dev mode, typed browser mocks (`src/lib/tauri-dev-mocks.ts`) stand in for server commands. Mock mode uses fixture data and never reads a kubeconfig or reaches a cluster. GPUI is the native desktop client.

Run every command from the repository root. `bun install` must have run in this checkout.

## Surfaces

| Surface | Use for | How | Status |
| --- | --- | --- | --- |
| Browser dev mode, `mock-dev` | UI behavior, layout, navigation, guarded-action UX | `kubecove-verify.ts` below | Primary. Proven by this skill. |
| Headless server and MCP (ADR 0020, ADR 0028) | HTTP boundary, browser sign-in, read-only MCP tools | `kubecove-verify-server.ts` | Runs locally with a fixture kubeconfig and no cluster. |
| GPUI desktop (ADR 0035) | Native commands, MCP and remote sign-in | `bun run e2e:desktop-smoke` | Uses a synthetic kubeconfig. See [GPUI desktop smoke](#gpui-desktop-smoke). |
| Kind lab (ADR 0012) | Real Kubernetes behavior | `bun run e2e:real`, `bun run dev:kind` | `e2e:real` runs locally with rootful podman and temporary firewall rules, which need the user's approval. See [Kind lab](#kind-lab). |

`bun run e2e:fast` runs the maintained WDIO specs in `e2e/specs/fast/` against the same mock frontend in Chrome. Use it as a regression gate, not as a way to show one journey.

## Launch

```sh
.agents/skills/verify-kubecove/scripts/kubecove-verify.ts launch
```

- Starts `bun --bun vite --host 127.0.0.1 --port <free port> --strictPort` in its own process group, with `KUBECONFIG` unset.
- Starts headless Chrome with a new profile in its own process group and connects over CDP. No window opens on the desktop.
- Ready means usable: the page carries the `html[data-kubecove-dev-mocks]` marker, which the browser mock client sets once it initializes. The command prints `"ready": true`, the URL, the evidence directory, and `export KUBECOVE_VERIFY_RUN=<id>`.
- Pass `--run <id>` to choose the run id. Later commands find the run on their own while exactly one run exists. Otherwise pass `--run <id>` or set `KUBECOVE_VERIFY_RUN`.
- On failure, or on Ctrl-C or SIGTERM before it is ready, launch stops what it started and keeps `vite.log` in the evidence directory.

Each run gets its own port and Chrome profile, leaving the user's `bun run dev` on port 1430 available. The helper drives only runs with a state file under `.e2e/verify/`. Leave any Vite server or KubeCove window you did not start running.

Chrome comes from `KUBECOVE_VERIFY_CHROME`, then Playwright's pinned Chromium, then the newest Chromium in Playwright's cache. If none exists, launch fails with the path it expected. Downloading one with `bunx playwright-core install chromium` writes to the user's cache, so ask the user first.

## Doctor

```sh
.agents/skills/verify-kubecove/scripts/kubecove-verify.ts doctor
```

Run this read-only check first if a step fails. It checks process identity by start time, the commit at launch, Vite socket ownership on Linux, the page title, CDP connectivity, and that the page carries the `html[data-kubecove-dev-mocks]` marker, which the browser mock client sets only when the mock runtime initializes. The check does not read view text. Start a new run after changing `HEAD`.

## Drive

```sh
.agents/skills/verify-kubecove/scripts/kubecove-verify.ts browser <action> [target] [options]
```

Targets use Playwright locators. Prefer roles and accessible names.

- `--role <role> [--name <name>] [--exact]`, `--label <label>`, `--text <text>`, or `--selector <css>`.
- `--within <css>` scopes the target, for example `--within 'tr:has(td:text-is("Deployment"))'`. `--nth <n>` picks a match; the default is the first.

Actions:

- `click`, `fill --value <text>`, `press --key <key>` (focused element unless a target is given), `wait [--state hidden]`.
- `count`, `value` (input value). Both print the result as JSON.
- `goto --path <path>`, `reload`, `reset` (clear `localStorage`, reload, wait for the `html[data-kubecove-dev-mocks]` marker), `viewport --width <px> --height <px>`.
- `snapshot --path <file>` writes an ARIA snapshot of the target or the whole page. `text --path <file>` writes inner text. `screenshot --path <file>` writes a PNG of the target, or of the viewport without one; `--full` captures the whole page and takes no target.
- `--timeout <ms>` overrides the default action timeout, which is the helpers' 30 s readiness timeout. It must be a positive number.

Each command prints `{action, title, url}` and appends the action, its arguments, and the resulting URL to `actions.jsonl` in the evidence directory. Read [features/README.md](features/README.md) and the matching feature file for exact journeys.

The default viewport is 1440 x 900, above the `xl` breakpoint, where the sidebar tree is visible. Use `viewport --width 768 --height 1024` for the tablet breakpoint and `--width 375 --height 812` for mobile. Below `xl`, open the sidebar with the `Open workspace navigation` button.

## Evidence

Evidence goes to `e2e/artifacts/verify-<run-id>/`, which Git ignores. Relative `--path` values resolve there. It holds `actions.jsonl`, `vite.log`, and every snapshot, text dump, and screenshot. Cleanup keeps it.

Proof standards:

- Drive the real user path: buttons, fields, and keys. Do not call stores, mocks, or `page.evaluate` setters to reach a state. `reset` is setup only.
- Capture the action and the resulting state. Take a snapshot before and after the step under test, not only at the end.
- Verify the side effect from a second view. After a mutation, reopen the resource or read the table, not only the success toast.
- Mutations in mock mode change in-memory mock state for this page only. A status that reads `in browser mock mode` proves the UI path, not a Kubernetes write. Prove cluster writes on the Kind lab.
- Record the feature ID and the entry point you used. Do not report an entry point as verified through a different one.

## Cleanup

```sh
.agents/skills/verify-kubecove/scripts/kubecove-verify.ts cleanup
```

Stops this run's Chrome and Vite process groups by recorded PID, waits until no process in either group remains, and deletes `.e2e/verify/<run-id>/` and its Chrome profile. It prints `evidenceKept: true` when the evidence directory still has `actions.jsonl`. Run cleanup after every failed attempt too.

Launch records each group leader's start time with its PID. When the recorded PID now belongs to a process with a different start time, or on Linux the record comes from an earlier boot, cleanup signals nothing, because the run's own processes are gone. When a group does not stop, a live leader's start time cannot be read, or the record has no start time while its group still runs, cleanup exits 1 with `stateKept: true` and keeps `.e2e/verify/<run-id>/` so it can be run again. On Windows a PID names one process, and `taskkill /T` finds children only through a live parent, so a leader that exited before cleanup, or whose PID now names another process, also keeps the state: its children may still run. Report the PID; do not kill by process name. Delete the run directory by hand only after the user confirms that the PID is gone or is not the run's.

## Helpers

- `scripts/kubecove-verify.ts`: launch, doctor, browser actions, and cleanup for browser dev mode.
- `scripts/kubecove-verify-server.ts`: a one-shot server and MCP check.
- `scripts/processes.ts`: shared port, process-group, and Chrome helpers. It is a module, not a command.

## Headless server and MCP

Build the browser runtime and the server once. Use a per-workspace cargo build dir if the user's cargo config shares one across workspaces:

```sh
KUBECOVE_PUBLIC_RUNTIME=web nice bun --bun vite build
(cd rust/server && CARGO_BUILD_BUILD_DIR="$PWD/target" nice cargo build --locked)
.agents/skills/verify-kubecove/scripts/kubecove-verify-server.ts
```

The check gives `kubecove-server` a private `HOME`, data directory, and a fixture kubeconfig selected through `KUBECOVE_VERIFY_CONFIG`, as `e2e/headless-smoke.ts` does, so the server never reads `~/.kube/config`. It starts `--listen 127.0.0.1:0 --mcp`, then checks:

- the server serves the embedded index;
- `/mcp` returns 401 without a token and with the browser server token;
- `tools/list` with the MCP token returns exactly the 11 tools ADR 0028 approves, each marked `readOnlyHint: true`. A new tool fails the check until the ADR and the list in the script are updated;
- `list_contexts` succeeds (`isError` is not true) and returns the fixture context `verify-fixture` as current;
- browser sign-in through `/#token=` reaches the launcher with `verify-fixture` selected.

It stops the server and Chrome by PID, deletes its scratch directory, and writes `result.json`, `server.log`, `browser-launcher.aria.txt`, and `browser-launcher.png` to `e2e/artifacts/verify-server-<run-id>/`. Ctrl-C or SIGTERM runs the same teardown and fails the check. If the server or Chrome does not stop, or the scratch directory cannot be removed, the check fails and keeps the scratch directory, which holds the tokens; `result.json` names it and both PIDs under `scratchKept`, and a removal error under `scratchError`. Stop those PIDs, then delete the directory. After both stop, it waits for the server's output to end before it writes `server.log`. If the output does not end within the readiness timeout or a read fails, the check fails and `result.json` records `outputComplete: false` or `outputErrors`. Startup and MCP tokens are replaced with `[redacted]` in every artifact. The fixture cluster address is a closed port, so namespace discovery fails in the launcher by design. Cluster-backed tools need the Kind lab.

## GPUI desktop smoke

Run `bun run e2e:desktop-smoke` in a separate graphical session. It builds the GPUI E2E flavor (ADR 0035) and the server, then drives the native client through its loopback test automation endpoint. It uses a synthetic kubeconfig and a local fixture server. The result is `e2e/artifacts/desktop-<run-id>/gpui-smoke.json`. CI runs this smoke on macOS and Windows.

## Kind lab

`bun run e2e:real` and `bun run dev:kind` use `e2e/lifecycle.ts`. It downloads checksum-pinned `kind`, `kubectl`, and `helm` into `.e2e/tools/`, so `kind` need not be on `PATH`. It picks `docker` or `podman` from `PATH`; a shell alias does not count. It writes a generated kubeconfig that holds only the lab's own contexts (`.e2e/runs/<id>/kubeconfig` or `.e2e/dev/<hash>/kubeconfig`), removes `KUBECONFIG` from child environments, and deletes only the exact cluster it recorded. Never point KubeCove or `kubectl` at another kubeconfig for verification.

`dev:kind` starts a loopback server and signs into the embedded Svelte UI in headless Chromium.

Cilium needs eBPF. With rootless podman and `kernel.unprivileged_bpf_disabled` set, the lab does not start, and a host firewall can block the Kind network. Both fixes change the live host, so ask the user before each run that needs them. Record a skipped lab run as unverified with the missing prerequisite.

With approval, a run on a host with passwordless sudo, ufw, and podman's `kind` network on `podman1` works like this:

1. Put a `podman` shim first on `PATH` in a scratch directory outside the repo. It holds `exec sudo -n /usr/bin/podman "$@"`. `podman info --format '{{.Host.Security.Rootless}}'` then prints `false`.
2. Allow the Kind network for this run only. First run `sudo ufw show added` and note which of these two rules already exist. Add only the missing ones: `sudo ufw allow in on podman1` and `sudo ufw route allow in on podman1 out <uplink>`.
3. Run `bun run e2e:real -- --provider podman --run-id <id>` with the shim on `PATH` and a lowered CPU priority. The default is Kubernetes 1.35. `--kubernetes <version>` selects another one from the support window in ADR 0011.
4. Afterwards, delete only the rules this run added, with `sudo ufw delete allow in on podman1` or `sudo ufw route delete allow in on podman1 out <uplink>`. A rule that existed before the run stays. Then compare `sudo ufw show added` with the list from step 2.
5. Confirm the cluster is gone with `sudo podman ps -a`. The lifecycle deletes only `kubecove-e2e-<id>`. Leave any other cluster, such as a `kubecove-dev-*` one, and the `kind` network in place.

Evidence goes to `e2e/artifacts/<id>/`: `wdio.log`, the per-spec logs, `kubeconfig-summary.json` (context names and a loopback endpoint, no credentials), `versions.txt`, and cluster diagnostics.

## Maintaining the map

The feature map in `features/` is the maintained source for what to drive. When a UI change renames a button, moves a view, or changes a journey, update the matching feature file in the same change.
