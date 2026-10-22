# Engineering and Governance

User-facing procedures start in the [public Wiki](https://github.com/Timpan4/kubecove/wiki). This directory contains engineering references, governance, and the source used to publish Wiki pages.

## Public product documentation

- [Getting started and downloading](https://github.com/Timpan4/kubecove/wiki/Install-and-Update)
- [Run KubeCove on a remote host](https://github.com/Timpan4/kubecove/wiki/Remote-Server)
- [Kubernetes resource browser and topology](https://github.com/Timpan4/kubecove/wiki/Resource-Browser)
- [Argo CD and Flux integration](https://github.com/Timpan4/kubecove/wiki/GitOps-Argo-CD-and-Flux)
- [Helm inspection and reconciliation](https://github.com/Timpan4/kubecove/wiki/Helm-Reconciliation)
- [Security, data handling, and architecture](https://github.com/Timpan4/kubecove/wiki/Safety-Data-and-Architecture)

## Engineering

- [Development Workflow](development-workflow.md) — setup, local development, and checks.
- [Legacy code and test audit](legacy-test-audit.md) — removal evidence, preserved coverage, and verification boundaries.
- [Rust build measurements](rust-build-measurements.md) — private-crate boundaries and incremental rebuild evidence.
- [CodSpeed benchmark catalog](codspeed-benchmark-catalog.md) — current coverage, benchmark candidates, fixtures, and measurement boundaries.
- [Built-in resource summary projection](resource-summary-projection.md) — shared list, details, and topology summary rules, kept differences, and measurements.
- [Deferred Svelte surfaces](lazy-surface-loading.md) — import boundaries and before/after bundle and native launcher evidence.
- [Engineering Handbook](handbook/README.md) — code organization, design system, hygiene, and pull request checklist.
- [Frontend memory characterization](frontend-memory-retention.md) — normal and large payload, retention, native process, and refetch measurements.
- [Backend cache diagnostics](backend-cache-diagnostics.md) — redacted cache states, counters, and retained payload measurements.
- [Headless memory optimization](headless-memory-optimization.md) — CRD allocation cause, Linux replay, output parity, and decoder benchmarks.
- [Release footprint variants](release-footprint-variants.md) — isolated stable Cargo comparisons and package, startup, font, benchmark, and crash evidence.
- [Product and architecture](product-and-architecture.md) — current product principles, desktop architecture, and trust boundaries.
- [Release Guide](release.md) — release preparation and publishing.
- [Headless server operations](wiki/Remote-Server.md) — supported remote-host setup and secure access.
- [Container and Kubernetes deployment](../deploy/README.md) — image build, RBAC choice, private k0s access, Cloudflare Tunnel and Access, and OIDC with Authentik.

## Governance

- [Architecture decisions](decisions/) — accepted technical and safety decisions.
- [Planning and milestones](milestones.md) — GitHub Issue and Milestone tracking direction.
- [Multiple user sessions and persistence](decisions/0023-multi-user-sessions-and-persistence.md) — implemented session baseline and proposed private-source provisioning, persistent user workspaces, settings and migration.
- [Single command contract](decisions/0025-single-command-contract.md) — proposed single declaration for Tauri IPC and server HTTP commands, and removal of the legacy name-only operation commands.
- [Cluster access by intent](decisions/0026-cluster-access-by-intent.md) — single module for Kubernetes clients named by intent, and the server's restriction to operator-configured kubeconfig sources.
- [Security Policy](../SECURITY.md) — vulnerability reporting and support.
- [OSS Scanner](../.oss-scanner/README.md) — offline audit environment, threat model, and maintainer enrollment.
- [Contributing](../CONTRIBUTING.md) — contribution and documentation reporting process.
- [License](../LICENSE) — AGPL-3.0-or-later terms.

## Wiki publishing

`docs/wiki/` is the canonical user-documentation source. A push to `main` that changes it runs `.github/workflows/publish-wiki.yml`, which replaces the Wiki repository contents with an exact mirror.

GitHub creates the separate Wiki Git repository only after its first page exists. Before the first authorized publish, a maintainer must create the initial Wiki **Home** page if the Wiki is still uninitialized. The first workflow run must also confirm that the repository `GITHUB_TOKEN` can push to the Wiki repository; this cannot be proven without publishing.
