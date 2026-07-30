# Engineering and Governance

User documentation starts in [public Wiki](https://github.com/Timpan4/kubecove/wiki).

## Engineering

- [Development Workflow](development-workflow.md) — setup, local development, and checks.
- [Engineering Handbook](handbook/README.md) — code organization, design system, hygiene, and pull request checklist.
- [Product and architecture](product-and-architecture.md) — current product principles, desktop architecture, and trust boundaries.
- [Release Guide](release.md) — release preparation and publishing.

## Governance

- [Architecture decisions](decisions/) — accepted technical and safety decisions.
- [Planning and milestones](milestones.md) — GitHub Issue and Milestone tracking direction.
- [Security Policy](../SECURITY.md) — vulnerability reporting and support.
- [Contributing](../CONTRIBUTING.md) — contribution and documentation reporting process.
- [License](../LICENSE) — AGPL-3.0-or-later terms.

## Wiki publishing

`docs/wiki/` is the canonical user-documentation source. A push to `main` that changes it runs `.github/workflows/publish-wiki.yml`, which replaces the Wiki repository contents with an exact mirror.

GitHub creates the separate Wiki Git repository only after its first page exists. Before the first authorized publish, a maintainer must create the initial Wiki **Home** page if the Wiki is still uninitialized. The first workflow run must also confirm that the repository `GITHUB_TOKEN` can push to the Wiki repository; this cannot be proven without publishing.
