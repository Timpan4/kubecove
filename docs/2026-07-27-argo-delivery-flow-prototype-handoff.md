# Argo delivery-flow prototype handoff

## Resume here

- Branch: `argo-delivery-flow-prototype`
- Current overview direction: **Delivery flow**
- Details-pane direction: **Briefing**, locked as the sole layout
- Prototype query parameter: `?argoOverviewPrototype=flow` before the hash route
- In development, the floating bottom control and Left/Right arrow keys switch between overview variants.

Use the `mock-dev` context and open the `platform-argocd` Application's resources to reach the overview header. The Argo CD tab on an Application resource opens the locked details pane.

## Validated design decisions

### Application details pane

Briefing won and is now the only details-pane layout. The old Compact alternative, selector, URL variation handling, and dashboard-variation session state were removed.

The locked pane keeps:

- compact health and sync briefing with local Refresh and Sync actions;
- reconciliation signal counts and condition warning;
- horizontal change navigator;
- delivery and recent-deployment context before the change stream;
- one continuous Changes, Desired, and Live stream;
- YAML syntax highlighting in Changes and read-only YAML editors in Desired and Live;
- removal resources visible in the stream, with their explanation always visible and live YAML collapsed behind a per-resource Show diff control.

### Resource overview

Three frontend-only comparison variants remain:

1. **Briefing bar** — mirrors the details pane in two rows.
2. **Signal deck** — gives health and reconciliation signals priority.
3. **Delivery flow** — leads with source, revision, and destination.

Delivery flow is the current preferred direction. Its lower rail is one continuous Argo-tinted surface. The resource total and reconciliation badges are integrated into that rail rather than placed in a detached dark panel.

Reconciliation badge tones now follow shared semantic status styles:

- Out of sync: warning/amber
- Degraded: error/red
- Progressing: warning/amber
- Prune: error/red
- All current: success/green

Do not remove the other two overview variants or the development-only switcher until the overview direction is explicitly locked.

## Prototype boundary

This remains deterministic frontend-only prototype work.

- Refresh, Hard refresh, Sync, advanced Sync, progress, confirmation, and completion are simulated locally.
- Do not add Rust, Tauri contracts, backend mutations, production authorization, persistence, real watches, or real Argo operations as part of this prototype.
- The comparison switcher is hidden in production builds.

## Important files

- `src/features/gitops/ArgoApplicationDetails.svelte` — locked Briefing details pane and continuous diff stream.
- `src/features/gitops/ArgoApplicationWorkspaceHeader.svelte` — three resource-overview variants and shared actions.
- `src/components/PrototypeSwitcher.svelte` — development-only floating comparison control.
- `src/features/gitops/argo-workspace-model.ts` — deterministic fixture, resource states, and sync rules.
- `src/features/gitops/argo-workspace-session.ts` — shared in-memory Application session state.
- `src/features/resource-detail/yamlTabDiff.ts` — Argo resource diff support.
- `src/features/resource-detail/ResourceDetailPanel.svelte` — hosts the Argo CD tab.
- `src/features/resources/ResourceBrowser.svelte` — hosts the Application overview header.
- `src/components/ToastViewport.svelte` and `src/lib/toasts.ts` — local prototype completion feedback.
- `tests/argo-workspace-model.test.ts` — fixture and state-model coverage.
- `docs/superpowers/specs/2026-07-25-argo-details-pane-prototype-design.md` — original prototype design question and boundaries.

## Verification completed

Final checks after the badge-tone correction:

```text
bun run typecheck
bun run svelte:check
```

Both passed; Svelte reported 0 errors and 0 warnings across 181 files.

Earlier targeted browser verification covered all three overview variants at 1440 px and 768 px, URL/switcher changes, Refresh and Sync menus, and the absence of page-level horizontal overflow. Instrumentation recorded no production Argo operation, inspector, comparison, connection, or watch calls. The final badge-tone-only correction was compile-checked without repeating the full browser suite.

## Recommended continuation

1. Open Delivery flow in the real resource overview and continue visual feedback there.
2. If Delivery flow is explicitly locked, remove Briefing bar, Signal deck, the overview variation session field, URL handling, and `PrototypeSwitcher` from the production branch.
3. Preserve the winning layout as normal application code rather than silently treating all prototype variants as production-ready.
4. Keep backend integration as a separate, explicitly scoped implementation task.
