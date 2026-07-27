# PR / Agent Checklist

Run before PR or completion.

## Verification

- [ ] Docs touched: `bun run docs:check` passes.
- [ ] Frontend touched: `bun run typecheck` passes.
- [ ] Frontend behavior touched: nearest `bun test` passes.
- [ ] Backend touched: `bun run rust:check` passes.
- [ ] Backend behavior touched: nearest `bun run rust:test` passes.
- [ ] Tauri integration touched: app starts and affected path works.
- [ ] Release flow touched: `bun run release:dry-run` passes.

## Organization and Hygiene

- [ ] Feature-specific frontend code lives in `src/features/<area>/`; generic UI or shared logic only in `src/components/` or `src/lib/`.
- [ ] New top-level directories are documented in [code-organization.md](code-organization.md).
- [ ] New Tauri commands have typed frontend wrappers; components do not call raw `invoke()`.
- [ ] Touched files meet size caps; oversized files have split follow-up.
- [ ] No superseded files, reference leftovers, commented-out blocks, or one-import wrappers remain.

## Tracking and Decisions

- [ ] Track planned or follow-up work in GitHub Issues; use Milestones for release or delivery grouping.
- [ ] Close or update linked GitHub Issues with delivered scope and deferred work.
- [ ] Security boundary changes have ADR.
- [ ] Live-session behavior follows ADR 0003; other cluster-changing behavior follows ADR 0004.
- [ ] Argo CD API/CLI, sync, rollback, or diff support has focused ADR coverage before implementation.

## Secrets

- [ ] No kubeconfig, token, certificate, or test-cluster credential is staged.
- [ ] Command payloads do not expose broad filesystem contents or raw Kubernetes credentials.
