# k8s-manager

A local desktop Kubernetes IDE experiment with a namespace-first and context-first workflow.

The goal is to build something similar in spirit to Lens or K8Studio, but with a workflow centered on:

- cluster groups and kubeconfig contexts
- namespaces as a first-class navigation surface
- app, owner, Argo CD, and Helm grouping signals
- read-only Kubernetes exploration first

This project is intentionally local-only for the MVP. Nothing is deployed into clusters, kubeconfig stays on the Rust side of the Tauri boundary, and normal Kubernetes API access should go through `kube-rs` rather than shelling out to `kubectl`.

## Planned Stack

- Tauri v2
- React and TypeScript
- Bun as the JavaScript runtime and package manager
- Rust backend
- `kube-rs` for Kubernetes API access
- TanStack Router, Query, and Table
- Zustand or Jotai for local UI state
- Tailwind CSS and shadcn/ui if it stays low-friction

Future candidates include Monaco Editor for YAML, xterm.js for pod exec, SQLite for local saved state, and optional sidecars or fallbacks for `kubectl`, Helm, and Argo CD.

## MVP Shape

The first working deliverable should:

- list local kubeconfig contexts
- select a context and list namespaces
- filter one or more namespaces globally
- list common read-only namespaced resources in a fast table
- show a read-only resource detail panel with YAML, metadata, and status

The second deliverable should add search, status chips, age formatting, owner detection, Argo/Helm label detection, empty/loading/error states, and persistent local UI selections.

## Docs

- [Architecture Blueprint](docs/architecture-blueprint.md)
- [Milestones](docs/milestones.md)
- [Project TODOs](docs/todos.md)
- [Product Inspiration](docs/product-inspiration.md)
- [Development Workflow](docs/development-workflow.md)
- [Architecture Decision Records](docs/decisions)
- [Agent Guide](AGENTS.md)

## Status

Repository foundation is being initialized. The desktop app scaffold has not been created yet.
