---
name: verify-kubecove
description: Drive KubeCove's Svelte workspace UI the way a user does and capture proof. The primary surface is browser dev mode with typed IPC mocks and the `mock-dev` context, driven by headless Chrome. Also covers the headless server with its MCP endpoint, the native desktop app under WDIO, and the Kind lab. Use it to confirm a frontend, workspace, resource-browser, detail-panel, guarded-action, incident, server, or MCP change works in the running app, not only in tests.
---

# Verify KubeCove

KubeCove is a Tauri v2 desktop app whose UI is a Svelte webview. In a plain browser, the Vite dev server runs the same UI with typed browser mocks in place of Tauri IPC (`src/lib/tauri-dev-mocks.ts`). Mock mode never reads a kubeconfig or reaches a cluster, so all data is fake. The IPC boundary is the production seam that isolates Rust and Kubernetes, so mocking there is allowed.

Run every command from the repository root. `bun install` must have run in this checkout.

## Surfaces

| Surface | Use for | How | Status |
| --- | --- | --- | --- |
| Browser dev mode, `mock-dev` | UI behavior, layout, navigation, guarded-action UX | `kubecove-verify.ts` below | Primary. Proven by this skill. |
| Headless server and MCP (ADR 0020, ADR 0028) | HTTP boundary, browser sign-in, read-only MCP tools | `kubecove-verify-server.ts` | Runs locally with a fixture kubeconfig and no cluster. |
| Native desktop app under WDIO (ADR 0010) | Real Tauri IPC and Rust commands | `bun run e2e:desktop-smoke` | Runs locally in a headless compositor with no cluster. See [Desktop app](#desktop-app-under-wdio). |
| Kind lab (ADR 0012) | Real Kubernetes behavior | `bun run e2e:real`, `bun run dev:kind` | Needs root podman and firewall changes. See [Kind lab](#kind-lab). |

`bun run e2e:fast` runs the maintained WDIO specs in `e2e/specs/fast/` against the same mock frontend in Chrome. Use it as a regression gate, not as a way to show one journey.

## Launch

```sh
.agents/skills/verify-kubecove/scripts/kubecove-verify.ts launch
```

- Starts `bun --bun vite --host 127.0.0.1 --port <free port> --strictPort` in its own process group, with `KUBECONFIG` unset.
- Starts headless Chrome with a new profile in its own process group and connects over CDP. No window opens on the desktop.
- Ready means usable: the launcher at `/#/workspaces` shows the `mock-dev` context. The command prints `"ready": true`, the URL, the evidence directory, and `export KUBECOVE_VERIFY_RUN=<id>`.
- Pass `--run <id>` to choose the run id. Later commands find the run on their own while exactly one run exists. Otherwise pass `--run <id>` or set `KUBECOVE_VERIFY_RUN`.
- On failure, launch stops what it started and keeps `vite.log` in the evidence directory.

Each run gets its own port and Chrome profile, so runs can go side by side, next to a user's `bun run tauri dev` on port 1430. The configured port 1430 (`vite.config.ts`, and `kubecove-browser-dev` in the main checkout's untracked `.claude/launch.json`) is never used. Never stop a Vite server on 1430 or a KubeCove window you did not start. The helper refuses to drive a run that has no state file under `.e2e/verify/`.

Chrome comes from `KUBECOVE_VERIFY_CHROME`, then Playwright's pinned Chromium, then the newest Chromium in Playwright's cache. If none exists, launch fails with the path it expected. Downloading one with `bunx playwright-core install chromium` writes to the user's cache, so ask the user first.

## Doctor

```sh
.agents/skills/verify-kubecove/scripts/kubecove-verify.ts doctor
```

Read-only. Exits 0 only when all checks pass: Vite and Chrome PIDs are alive, `HEAD` matches the commit at launch, the listening socket belongs to this run's Vite process group (Linux), the index has `<title>KubeCove</title>`, CDP answers, and the page has no Tauri internals, so it runs browser mocks. Run it first whenever a step fails or anything looks wrong. A changed `HEAD` means the UI under test no longer matches the run, so launch a new one.

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
- `goto --path <path>`, `reload`, `reset` (clear `localStorage`, reload, wait for `mock-dev`), `viewport --width <px> --height <px>`.
- `snapshot --path <file>` writes an ARIA snapshot of the target or the whole page. `text --path <file>` writes inner text. `screenshot --path <file> [--full]` writes a PNG.
- `--timeout <ms>` overrides Playwright's default action timeout.

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

Stops this run's Chrome and Vite process groups by recorded PID, waits for them to exit, and deletes `.e2e/verify/<run-id>/` and its Chrome profile. It prints `evidenceKept: true` when the evidence directory still has `actions.jsonl`. Run cleanup after every failed attempt too. If it reports a group that did not stop, report the PID; do not kill by process name.

## Helpers

- `scripts/kubecove-verify.ts`: launch, doctor, browser actions, and cleanup for browser dev mode.
- `scripts/kubecove-verify-server.ts`: a one-shot server and MCP check.
- `scripts/processes.ts`: shared port, process-group, and Chrome helpers. It is a module, not a command.

## Headless server and MCP

Build the browser runtime and the server once. Use a per-workspace cargo build dir if the user's cargo config shares one across workspaces:

```sh
KUBECOVE_PUBLIC_RUNTIME=web nice bun --bun vite build
(cd src-tauri/server && CARGO_BUILD_BUILD_DIR="$PWD/target" nice cargo build --locked)
.agents/skills/verify-kubecove/scripts/kubecove-verify-server.ts
```

The check gives `kubecove-server` a private `HOME`, data directory, and a fixture kubeconfig selected through `KUBECOVE_VERIFY_CONFIG`, as `e2e/headless-smoke.ts` does, so the server never reads `~/.kube/config`. It starts `--listen 127.0.0.1:0 --mcp`, then checks:

- the server serves the embedded index;
- `/mcp` returns 401 without a token and with the browser server token;
- `tools/list` with the MCP token returns only tools marked `readOnlyHint: true`;
- `list_contexts` returns the fixture context `verify-fixture`;
- browser sign-in through `/#token=` reaches the launcher with `verify-fixture` selected.

It stops the server and Chrome by PID, deletes its scratch directory, and writes `result.json`, `server.log`, `browser-launcher.aria.txt`, and `browser-launcher.png` to `e2e/artifacts/verify-server-<run-id>/`. Startup and MCP tokens are replaced with `[redacted]` in every artifact. The fixture cluster address is a closed port, so namespace discovery fails in the launcher by design. Cluster-backed tools need the Kind lab.

## Desktop app under WDIO

`bun run e2e:desktop-smoke` builds the E2E-only Tauri flavor (`--features e2e`, ADR 0010) and the server, then runs `e2e/specs/desktop/` against the native app through the embedded WebDriver on port 4445. It uses a synthetic loopback kubeconfig and no cluster. Artifacts go to `e2e/artifacts/desktop-<run-id>/`. Pass `-- --spec <file>` for one spec.

On Linux it opens a native window, so never run it on the user's desktop. Run it inside a headless compositor, with a private D-Bus session and keyring. On a workstation with `cage` and a GPU render node:

```sh
dbus-run-session -- sh -c '
  eval "$(printf kubecove-e2e | XDG_DATA_HOME=<tmp>/data gnome-keyring-daemon --daemonize --unlock --components=secrets --control-directory=<tmp>/kr)"
  env -u DISPLAY GDK_BACKEND=wayland WLR_BACKENDS=headless WLR_RENDERER=gles2 WLR_RENDER_DRM_DEVICE=/dev/dri/renderD128 WLR_LIBINPUT_NO_DEVICES=1 \
    cage -- sh -c "wlr-randr --output HEADLESS-1 --custom-mode 1024x720 && nice bun run e2e:desktop-smoke"
  kill "$GNOME_KEYRING_PID"'
```

`<tmp>` is a new 0700 directory. Pass a non-empty unlock password: an empty one creates no keyring. Kill the keyring daemon by PID afterwards, because it keeps the session alive. On a CI-like host, `xvfb-run -a` replaces `cage`, as `docs/development-workflow.md` shows.

## Kind lab

`bun run e2e:real` and `bun run dev:kind` use `e2e/lifecycle.ts`. It downloads checksum-pinned `kind`, `kubectl`, and `helm` into `.e2e/tools/`, so `kind` need not be on `PATH`. It picks `docker` or `podman` from `PATH`; a shell alias does not count. It writes a generated kubeconfig that holds only the lab's own contexts (`.e2e/runs/<id>/kubeconfig` or `.e2e/dev/<hash>/kubeconfig`), removes `KUBECONFIG` from child environments, and deletes only the exact cluster it recorded. Never point KubeCove or `kubectl` at another kubeconfig for verification.

`dev:kind` starts `bun run tauri dev`, which opens a native window, so wrap it like the desktop app. It refuses to start while anything answers on port 1430.

Cilium needs eBPF. With rootless podman and `kernel.unprivileged_bpf_disabled` set, the lab does not start, and a host firewall can block the Kind network. Both fixes change the live host, so ask the user before each run that needs them. Record a skipped lab run as unverified with the missing prerequisite.

## Maintaining the map

The feature map in `features/` is the maintained source for what to drive. When a UI change renames a button, moves a view, or changes a journey, update the matching feature file in the same change.
