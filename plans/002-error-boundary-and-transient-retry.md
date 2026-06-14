# Plan 002: Add React error boundaries and stop retrying deterministic query errors

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat 9da3831..HEAD -- src/main.tsx src/App.tsx src/components/ src/lib/tauri.ts`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: S
- **Risk**: LOW
- **Depends on**: none
- **Category**: bug
- **Planned at**: commit `9da3831`, 2026-06-10

## Why this matters

KubeCove is a Tauri desktop app whose views are all lazy-loaded under
`Suspense`, but there is **no ErrorBoundary anywhere in `src/`**. A render
error in any panel — or a failed lazy-chunk load after an auto-update —
white-screens the entire app. Separately, the global TanStack Query default
`retry: 2` retries *every* failed query, including deterministic Kubernetes
errors (RBAC forbidden, not-found, validation): the user waits through three
identical failures before seeing the error. Both fixes are small and make the
app dramatically more robust during incidents, which is exactly when it's
used.

## Current state

- `src/main.tsx` (28 lines) — app entry; the query client:

```tsx
// src/main.tsx:10-17
const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 30_000,
      retry: 2,
    },
  },
});
```

- `src/App.tsx` (579 lines) — shell. The two large render regions are built
  as elements and passed to `DetailPanelFrame`:

```tsx
// src/App.tsx:473-475
const mainContent = (
    <AppMainContent
        ...
// src/App.tsx:514-516
const detailPanel = hasAppDetailPanel(viewMode, selectedHelmRelease !== null, selectedArgoApp !== null, selectedResource !== null) ? (
    <AppDetailPanel
        ...
// src/App.tsx:568-571
<DetailPanelFrame
    mainContent={mainContent}
    detailPanel={detailPanel}
/>
```

- Backend errors cross the Tauri IPC boundary as a serialized `AppError`
  struct — a plain object `{ message: string, kind: string }`
  (`src-tauri/src/models/error.rs:3-7`, frontend type
  `src/lib/types.ts:458-461`). The `kind` values used by the backend
  (verified by grep over `src-tauri/src`): `"cluster"` (all kube errors,
  including transient network failures AND deterministic 403/404),
  `"validation"`, `"serialization"`, `"session"`, `"io"`,
  `"fieldManagerConflict"`.
- `src/lib/tauri.ts:48-59` has a private `errorMessage(error: unknown)`
  helper showing the repo's error-narrowing style — match it.
- Conventions: generic reusables go in `src/components/` (per
  `docs/handbook/code-organization.md`); indentation is tabs in `src/`
  (see any file, e.g. `src/lib/hooks.ts`); `.tsx` soft cap 400 lines;
  frontend must pass `bun run typecheck`; tests are bun tests in `tests/`
  (pure-logic style — see `tests/log-helpers.test.ts` for the pattern:
  `import { describe, expect, test } from "bun:test"`).

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Typecheck | `bun run typecheck` | exit 0 |
| Frontend tests | `bun test` | all pass |

## Scope

**In scope** (the only files you should modify/create):
- `src/components/ErrorBoundary.tsx` (create)
- `src/lib/query-retry.ts` (create)
- `src/main.tsx` (edit: retry option)
- `src/App.tsx` (edit: wrap the two regions)
- `tests/query-retry.test.ts` (create)

**Out of scope** (do NOT touch):
- `src/app/lazyViews.ts`, `ViewLoadingFallback.tsx` — Suspense handling
  stays as is; the boundary wraps it, it doesn't replace it.
- Per-query `retry` overrides inside features — defaults only.
- `src/lib/tauri.ts` — do not export its private `errorMessage`; the retry
  predicate lives in its own module to avoid import cycles.
- Any styling system changes; use existing Tailwind utilities.

## Git workflow

- Branch: `fix/error-boundary-transient-retry`.
- If the current branch is `gitbutler/workspace`, all git writes must use
  `but`, never raw `git commit` (AGENTS.md "GitButler Workflow"). In an
  isolated worktree, normal git applies.
- GitButler baseline as of 2026-06-14: unassigned changes include the plan
  backlog files, maintainer backlog docs, Rust formatting-only edits, and
  `tweet-drafts.md`. The Rust formatting-only edits may be folded into the
  Plan 002 branch if they remain behavior-neutral:
  `src-tauri/src/commands/discovery.rs`,
  `src-tauri/src/commands/helm/storage.rs`,
  `src-tauri/src/commands/helpers/client_cache.rs`,
  `src-tauri/src/commands/kubeconfig.rs`,
  `src-tauri/src/commands/live_store.rs`,
  `src-tauri/src/commands/metrics.rs`,
  `src-tauri/src/commands/namespaces.rs`,
  `src-tauri/src/commands/resources/apply.rs`,
  `src-tauri/src/commands/sessions/service.rs`, and
  `src-tauri/src/commands/streams/registry.rs`.
- Do not fold the maintainer backlog docs, other plan backlog files, or
  `tweet-drafts.md` into the Plan 002 implementation branch. The only
  existing unassigned non-Rust files that may travel with Plan 002
  bookkeeping are this plan file and the status row in `plans/README.md`.
- Before writing code, run `but status -fv`. If any Plan 002 implementation
  file is already unassigned (`src/main.tsx`, `src/App.tsx`,
  `src/components/ErrorBoundary.tsx`, `src/lib/query-retry.ts`,
  `tests/query-retry.test.ts`), inspect and incorporate that change instead
  of overwriting it.
- Commit style (from `git log`): `🐛fix: <one concrete outcome>`.
- Do NOT push or open a PR unless the operator instructed it.

## Steps

### Step 1: Create the retry predicate

Create `src/lib/query-retry.ts` (tabs, `.ts` cap 300 lines — this will be ~40):

```ts
const DETERMINISTIC_KINDS = new Set([
	"validation",
	"serialization",
	"fieldManagerConflict",
]);

const DETERMINISTIC_MESSAGE = /\bforbidden\b|\b403\b|\bnot ?found\b|\b404\b/i;

export function isTransientQueryError(error: unknown): boolean {
	if (typeof error === "object" && error !== null && "kind" in error) {
		const kind = (error as { kind?: unknown }).kind;
		if (typeof kind === "string" && DETERMINISTIC_KINDS.has(kind)) {
			return false;
		}
	}
	const message =
		error instanceof Error
			? error.message
			: typeof error === "object" && error !== null && "message" in error
				? String((error as { message?: unknown }).message ?? "")
				: String(error);
	return !DETERMINISTIC_MESSAGE.test(message);
}

export function queryRetry(failureCount: number, error: unknown): boolean {
	return isTransientQueryError(error) && failureCount < 2;
}
```

Rationale to preserve in a short comment: backend `kind` is coarse —
`"cluster"` covers both transient network errors and deterministic 403/404,
so the predicate also sniffs the message for forbidden/not-found.

**Verify**: `bun run typecheck` → exit 0

### Step 2: Wire the predicate into the query client

In `src/main.tsx`, replace `retry: 2` with `retry: queryRetry` (import from
`./lib/query-retry`).

**Verify**: `bun run typecheck` → exit 0

### Step 3: Create the ErrorBoundary component

Create `src/components/ErrorBoundary.tsx` — a class component (React error
boundaries must be classes), React 19, tabs:

- Props: `{ label: string; children: ReactNode }` — `label` names the region
  ("main content", "detail panel") in the fallback text.
- State: `{ error: Error | null }` via `static getDerivedStateFromError`.
- `componentDidCatch(error, info)`: log through the existing diagnostics
  channel: `diagnosticLog("app.render.error", { label, error: error.message })`
  (import from `@/lib/diagnostics`; check its signature —
  `src/lib/diagnostics.ts` — and match it).
- Fallback UI: a centered card with the label, the error message, and a
  "Try again" button that resets state (`this.setState({ error: null })`).
  Style with existing Tailwind utility classes (match the look of existing
  empty/error states — see how `AppMainContent` renders alerts). No new CSS
  in `App.css`.

**Verify**: `bun run typecheck` → exit 0

### Step 4: Wrap the two shell regions in App.tsx

In `src/App.tsx`, wrap the element trees:

```tsx
const mainContent = (
	<ErrorBoundary label="main content">
		<AppMainContent ... />
	</ErrorBoundary>
);

const detailPanel = hasAppDetailPanel(...) ? (
	<ErrorBoundary label="detail panel">
		<AppDetailPanel ... />
	</ErrorBoundary>
) : null;
```

Do not move any props or other logic. Also wrap the `<LauncherShell ...>`
early-return the same way (`label="launcher"`).

**Verify**: `bun run typecheck` → exit 0, then `bun test` → all pass

## Test plan

Create `tests/query-retry.test.ts`, modeled structurally on
`tests/log-helpers.test.ts` (bun test, pure functions). Cases:

1. `isTransientQueryError({ message: "x", kind: "validation" })` → `false`
2. same for `"serialization"` and `"fieldManagerConflict"` → `false`
3. `{ message: "deployments.apps is forbidden: User ...", kind: "cluster" }`
   → `false` (message sniff)
4. `{ message: 'pods "api-0" not found', kind: "cluster" }` → `false`
5. `{ message: "connection refused", kind: "cluster" }` → `true`
6. `new Error("error trying to connect: timed out")` → `true`
7. `queryRetry(2, transientError)` → `false` (count cap still applies)

The ErrorBoundary itself is not unit-tested (the repo has no component-test
infrastructure; do not introduce one here — that is a separate, larger
decision).

**Verification**: `bun test` → all pass including the new file.

## Done criteria

Machine-checkable. ALL must hold:

- [ ] `bun run typecheck` exits 0
- [ ] `bun test` exits 0; `tests/query-retry.test.ts` exists with the 7 cases
- [ ] `grep -n "retry: 2" src/main.tsx` returns no matches
- [ ] `grep -c "ErrorBoundary" src/App.tsx` ≥ 3 (launcher, main, detail)
- [ ] `git status` shows changes only in the 5 in-scope files
- [ ] `plans/README.md` status row updated

## STOP conditions

Stop and report back (do not improvise) if:

- `src/lib/diagnostics.ts`'s `diagnosticLog` signature doesn't accept
  `(name, payload)` — report instead of inventing a new logging path.
- Wrapping causes a typecheck error rooted in `DetailPanelFrame`'s prop types
  (it should accept any `ReactNode`; if it doesn't, report).
- You are tempted to convert ErrorBoundary to a third-party dependency
  (e.g. `react-error-boundary`) — new deps need owner sign-off.
- `App.tsx` exceeds 700 lines after the change (hard cap; it's at 579, the
  wrappers add ~6 lines).

## Maintenance notes

- When the backend grows distinct error kinds for forbidden/not-found
  (instead of overloading `"cluster"`), simplify
  `isTransientQueryError` to a pure kind check and delete the message regex —
  it's the fragile part.
- Reviewer should scrutinize: the regex (false positives would suppress
  useful retries — e.g. a pod literally named "notfound" appearing in a
  message is acceptable collateral; a cluster being temporarily 403 during
  token refresh is the riskier case, mitigated because the user can refetch).
- Deferred: per-view granular boundaries (one per lazy view) — start coarse;
  add finer boundaries only if a single panel crashing while others work
  becomes a real scenario.
