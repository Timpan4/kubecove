# Svelte Frontend Migration Plan

## Summary

Migrate KubeCove from React to Svelte only if Svelte proves app-wide value. The work starts as a parallel Svelte app behind a Settings toggle, then ships Svelte as the default when parity gates pass. React remains the fallback for one release.

Do not tune the topology spike further before the migration plan starts. Use the spike as evidence, then measure the full app.

## Work Slices

1. ADR and baseline
   - Land ADR 0008.
   - Record React baseline checks and perf numbers.
   - Keep the topology spike results as supporting evidence, not the migration decision by itself.

2. Parallel Svelte shell
   - Add the Svelte app entrypoint behind a UI mode setting.
   - Keep React available as the fallback while Svelte earns default status.
   - Add the runtime identity badge to both app shells.
   - Add the Settings UI mode toggle, toast, and clean reload behavior.

3. Shared contracts and state
   - Preserve existing local settings and workspace storage schemas.
   - Share typed Tauri wrappers, TypeScript types, query keys, and pure helpers.
   - Add Svelte stores or adapters for dashboard selection, foreground loading, diagnostics, settings, and workspaces.
   - Use Svelte Query for server/cache state.

4. Daily core parity
   - Port launcher/workspaces.
   - Port top bar, sidebar, main/detail frame, and settings.
   - Port resource list, resource detail, YAML, events, metrics, and topology.
   - Port GitOps and Helm read paths.
   - Port live-session list/status surfaces without changing live-session commands.

5. Tail parity and removal
   - Port command palette, incidents, RBAC, app updates, usage footer, and edge states.
   - Keep React fallback available for unported areas until this slice is complete.
   - Remove React runtime, React-only dependencies, React compiler config, React Doctor scripts, and obsolete React source only after tail parity passes.

## Current Checklist

Status date: 2026-06-22. Checked items are based on repo inspection and automated checks, not full manual cluster smoke unless explicitly stated.

- [x] Svelte app entrypoint, Settings runtime toggle, reload notice, and runtime badge exist.
- [x] React fallback remains selectable through the same runtime setting.
- [x] Settings and workspace storage schemas are shared across both runtimes.
- [x] Svelte launcher, workspace shell, sidebar, command palette, settings, update status, and usage footer exist.
- [x] Svelte resource browser, resource detail, YAML, events, logs, exec, port-forward, and topology surfaces exist.
- [x] Svelte GitOps, Helm, RBAC, incidents, and live-session list/status surfaces exist.
- [x] Automated evidence recorded on 2026-06-22: `bun run svelte:check`, `bun run typecheck`, and `bun test` pass.
- [x] Default runtime gate reconciled: new installs default to React until Svelte cutover evidence is complete.
- [ ] Record browser-backed app-wide launch, resource list, YAML detail, and 4k topology measurements.
- [ ] Prove at least one major app-wide memory or CPU-heavy bottleneck improves by 25% or more.
- [ ] Run manual Svelte and React fallback Tauri smoke against a readable cluster.
- [ ] Make Svelte the default only after daily core parity, perf gates, runtime toggle, and fallback smoke pass.
- [ ] Remove React only after tail parity and one-release fallback period complete.

## Gates

Continue beyond early Svelte slices only if:

- Svelte improves at least one major app-wide memory or CPU-heavy bottleneck by 25% or more.
- No daily core workflow regresses compared with React.
- The React fallback remains reliable.

Make Svelte the default only if:

- Daily core parity passes.
- Launch, resource list, YAML detail, and 4k topology are better than or neutral to React.
- The runtime badge clearly shows whether React or Svelte is active.
- Settings toggle reloads into the chosen UI mode cleanly.

Remove React only if:

- Tail parity is complete.
- Both user data schemas and persisted settings remain readable by Svelte.
- No Tauri command contract changed for the migration.

## Verification

Baseline React before migration:

```sh
bun run typecheck
bun run lint
bun test
bun run build
bun run doctor:score
bun --expose-gc scripts/perf-frontend.ts
```

During parallel migration:

```sh
bun run typecheck
bun run lint
bun test
bun run build
svelte-check
```

Manual smoke for each vertical slice:

- Runtime badge shows the active framework.
- Settings toggle shows a toast and reloads to the selected framework.
- Workspaces and selected scope survive switching frameworks.
- React fallback can open any unported area.
- Resource browsing, YAML, topology, GitOps/Helm read paths, and live-session list/status match React behavior before Svelte default.

## Defaults

- Use Svelte SPA under Vite, not SvelteKit, Astro, or a new router unless a later ADR changes this.
- Use Bits UI for Svelte headless primitives and local KubeCove styling wrappers.
- Use official React and Svelte logo assets as small local SVGs after checking license/usage requirements. Do not fetch logos at runtime.
- Keep all Kubernetes access behind typed Tauri wrappers.
- Do not add cluster mutation as part of frontend migration.
