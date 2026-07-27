# Development Workflow

## Package Manager

Use Bun for frontend dependencies and scripts. Use version pinned by CI; dependency versions belong in lockfiles and manifests, not this guide.

```sh
bun install
bun run tauri dev
bun run typecheck
bun test
bun run docs:check
bun run check
```

## Browser Dev Mock Mode

`bun run tauri dev` starts Vite at `http://localhost:1430`. Opening it in a normal browser runs Svelte with browser-only mock Tauri responses for frontend inspection and automation.

Tauri webview uses real IPC and Rust Kubernetes commands. On Windows, development exposes Chrome DevTools Protocol at `http://127.0.0.1:9222`; set `KUBECOVE_DEVTOOLS_PORT` before launch to change it. Packaged apps do not expose this endpoint.

Browser mock mode never receives kubeconfig contents, calls a local Rust bridge, or accesses a real cluster. Treat all browser data as fake.

## Deterministic E2E and Kind Lab

Use fast E2E while changing frontend behavior:

```sh
bun run e2e:fast
```

It starts Vite, opens Chrome through WDIO, and uses typed development-browser mocks. It neither builds Rust nor contacts Kubernetes.

Use real E2E for native-command and cluster behavior:

```sh
bun run e2e:real -- --provider auto
bun run e2e:real -- --keep
bun run e2e:cleanup -- --run-id <id>
```

Runner downloads checksum-verified tools and assets, creates one uniquely named Kind cluster, builds E2E-only Tauri flavor, and runs native WDIO serially. It bootstraps production-shaped Cilium, Git, Argo CD, Helm, and tenant fixtures; fake GitOps CRDs and fabricated status are not fixtures.

Runner gathers allowlisted redacted diagnostics, then deletes only its exact cluster on success, failure, SIGINT, or SIGTERM. Local `--keep` preserves exact run for inspection and is rejected in CI. Cleanup requires recorded exact run ID; it never deletes by prefix or reads user kubeconfig.

Generated E2E kubeconfig contains only dedicated contexts. Startup rejects non-absolute paths, unexpected contexts, non-loopback API servers, cluster mismatches, persisted sources, and fallback to user default kubeconfig. Artifacts never include raw kubeconfig, tokens, keys, or certificate data.

For manual development against same fixtures:

```sh
bun run dev:kind
bun run dev:kind:down
bun run e2e:desktop-smoke
```

`dev:kind` uses a workspace-derived cluster name and temporary app profile. Cluster remains until `dev:kind:down`; app profile disappears on exit.

See [ADR 0010](decisions/0010-e2e-only-wdio-security-boundary.md), [ADR 0011](decisions/0011-rolling-kubernetes-support.md), and [ADR 0012](decisions/0012-production-shaped-e2e-lab.md).

## Verification Before Completion

Run smallest checks matching work:

- docs-only: `bun run docs:check` and `git diff --check`
- frontend: `bun run typecheck` and nearest `bun test`
- backend: `bun run rust:check` and nearest `bun run rust:test`
- behavior change: nearest focused test
- Tauri integration: `bun run tauri dev` or smoke test
- release change: `bun run release:dry-run`
- guarded operation: relevant ADR checklist plus nearest frontend/backend checks

If a check cannot run, record exact blocker and remaining unverified behavior.
