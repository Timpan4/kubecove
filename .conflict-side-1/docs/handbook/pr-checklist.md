# PR / Agent Checklist

Run this before opening a PR or marking agent work complete.

## Verification

- [ ] Frontend touched: `bun run typecheck` passes.
- [ ] Frontend behavior touched: relevant `bun test` coverage passes.
- [ ] Backend touched: `bun run rust:check` passes.
- [ ] Backend behavior touched: relevant `bun run rust:test` coverage passes.
- [ ] Tauri integration touched: app starts and the affected path works.
- [ ] Release flow touched: `bun run release:dry-run` passes.

## Organization

- [ ] Feature-specific frontend code lives in `src/features/<area>/`.
- [ ] Generic frontend code only lives in `src/components/` or `src/lib/` when it is genuinely reusable.
- [ ] New feature folders or top-level directories are documented in [code-organization.md](code-organization.md).
- [ ] New Tauri commands have typed wrappers in `src/lib/tauri.ts`.
- [ ] Components do not call raw `invoke()` directly.

## Size

- [ ] No touched file exceeds the hard cap.
- [ ] Any touched file over the soft cap has a split plan or follow-up.
- [ ] No new file was added to `LEGACY_OVERSIZED_FILES`.

## Hygiene

- [ ] Superseded files were deleted.
- [ ] No "for reference" leftovers were added.
- [ ] No commented-out blocks or one-import wrappers were added.
- [ ] No `#[allow(dead_code)]` was added without a written reason.

## Tracking and Decisions

- [ ] Completed milestone items are checked in [milestones.md](../milestones.md).
- [ ] Security boundary changes have an ADR.
- [ ] Live-session behavior follows ADR 0003.
- [ ] Other cluster-changing behavior follows ADR 0004.
- [ ] Argo CD API/CLI, sync, rollback, or diff support has focused ADR coverage before implementation.

## Secrets

- [ ] No kubeconfig, token, certificate, or test-cluster credential is staged.
- [ ] New command payloads do not expose broad filesystem contents or raw Kubernetes credentials.
