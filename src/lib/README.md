# `src/lib/`

Pure shared logic belongs here. No JSX.

Allowed:

- typed Tauri wrappers in `tauri.ts`
- frontend mirrors of serde contracts in `types.ts`
- the Zustand settings store in `settings.ts` and its Svelte adapter in `settings-store.ts`
- the query catalog in `queryKeys.ts` and the finite-read lifecycle in `finite-read-lifecycle.ts`
- the Argo Application inspection in `argo-application-inspection.ts` and the live view refresh binding in `live-view-refresh.ts`, each with a component adapter in a matching `.svelte.ts` file; an adapter only binds its framework-free module to the calling component's lifecycle
- workspace, settings, release-channel, diagnostics, and formatting helpers
- pure resource health, metrics, visuals, and tree-navigation logic
- future guarded operation helpers when they are pure and shared

Not allowed:

- UI components
- imports from `src/features/`
- imports from `src/components/`
- logic that only one feature uses
- raw Kubernetes access, raw `invoke()` call sites outside `tauri.ts`, or shell command execution

Feature-only logic belongs beside that feature. Shared UI helpers belong in `src/components/` or `src/lib/`.

Caps: `.ts` soft 300 / hard 600. See [docs/handbook/file-size-and-split.md](../../docs/handbook/file-size-and-split.md).
