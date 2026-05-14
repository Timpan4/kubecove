# PR / Agent Checklist

Run through this before opening a PR or marking an agent task done. The pre-commit hook covers some of it; the rest is on you.

## Verification

- [ ] `bun run typecheck` passes (frontend touched).
- [ ] `bun run lint` passes (once lint exists).
- [ ] `cargo check --manifest-path src-tauri/Cargo.toml` passes (backend touched).
- [ ] The app starts and the affected feature works (Tauri integration changes).

## Organization

- [ ] No new file lives in `src/components/` if it's feature-specific. See [code-organization.md](code-organization.md).
- [ ] No new feature folder was created without a one-line entry in [code-organization.md](code-organization.md).
- [ ] Typed Tauri wrappers in `src/lib/tauri.ts` cover any new commands — no raw `invoke()` calls in components.

## Size

- [ ] No staged file exceeded its **hard** cap. (Hook fails if it did.)
- [ ] If a staged file crossed a **soft** cap, you have a plan or a follow-up issue. See [file-size-and-split.md](file-size-and-split.md).
- [ ] No new file was added to `LEGACY_OVERSIZED_FILES` in `.githooks/pre-commit-user`.

## Hygiene

- [ ] Any superseded file was deleted in this PR. See [hygiene.md](hygiene.md).
- [ ] No "for reference" leftovers, commented-out blocks, or one-import wrappers added to dodge a cap.
- [ ] No `#[allow(dead_code)]` added without a written reason.

## Tracking

- [ ] If this completes a milestone item, the box in [milestones.md](../milestones.md) is ticked.
- [ ] If a security boundary changed (frontend/backend split, kubeconfig handling, Tauri capabilities, mutation support, Argo CD CLI/API), an ADR was added under [decisions/](../decisions/).

## Secrets

- [ ] No kubeconfig, token, certificate, or test-cluster credential staged. The hook checks; spot-check too.
