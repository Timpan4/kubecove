# KubeCove

KubeCove is a local desktop workspace for Kubernetes operations. It starts from contexts, namespaces, and saved workspaces instead of raw resource-kind menus, then lets operators move quickly into resource tables, topology, Argo CD signals, Helm metadata, events, logs, YAML, and read-only detail views.

The app is local-first and read-only by default. Kubernetes access stays behind Rust-side Tauri commands, kubeconfig contents never cross into React, and the core data path uses `kube-rs` rather than shelling out to `kubectl`.

## Quick Start

Use the beta installers from [GitHub Releases](https://github.com/Timpan4/kubecove/releases) when you only want to test the app.

Development requirements:

- Bun
- Rust and Cargo
- Tauri v2 system prerequisites for your OS
- A local kubeconfig with at least one readable context

Run the desktop app:

```sh
bun install
bun run tauri dev
```

Useful checks:

```sh
bun run typecheck
bun test
bun run rust:check
bun run check
```

Build a desktop bundle:

```sh
bun run tauri build
```

Create a beta release tag from the current `origin/main` release version:

```sh
bun run release
```

## Current Capabilities

- List local kubeconfig contexts without exposing raw kubeconfig data.
- Create, edit, delete, and restore local workspace scopes.
- Browse namespaces and discovered Kubernetes resource kinds.
- Filter resource tables by namespace, kind, health, search text, Argo CD app, and owner context.
- Inspect resources through read-only details, YAML, events, logs, metrics, and topology views.
- Detect Argo CD CRDs and browse Applications, ApplicationSets, and AppProjects.
- Detect Helm releases from cluster metadata.
- Inspect RBAC summaries and risk indicators.
- Publish unsigned beta installers for macOS, Windows, and Linux.

## Safety Model

KubeCove does not deploy agents into clusters. It does not expose raw kubeconfig contents to the frontend, and React cannot run arbitrary shell commands.

Mutation workflows such as apply, delete, scale, sync, rollback, exec, or port-forward require a new Architecture Decision Record and explicit UX guardrails before they can become product features.

## Stack

- Tauri v2
- React and TypeScript
- Bun for JavaScript package management and scripts
- Rust backend
- `kube-rs` for Kubernetes API access
- TanStack Router, Query, and Table
- Zustand for local UI state
- Tailwind CSS and shadcn/ui primitives
- React Flow for topology surfaces

## Docs

Start with the [docs index](docs/README.md). The highest-signal entry points are [Product Vision](docs/product-vision.md), [Architecture Blueprint](docs/architecture-blueprint.md), [Milestones](docs/milestones.md), and [AGENTS.md](AGENTS.md).

## License

KubeCove is licensed under the [GNU Affero General Public License v3.0 or later](LICENSE).
