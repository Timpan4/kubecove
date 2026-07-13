# Deep Module Refactor Design

**Date:** 2026-07-12
**Status:** Approved

## Objective

Deepen three existing KubeCove modules without speculative seams:

1. Port Forward Lifecycle
2. Workspace Navigation
3. Incident signal presentation

The refactor preserves behavior by default. It may correct an inconsistency only when a focused test or concrete reproduction proves that two existing paths disagree.

## Delivery strategy

Implement the candidates sequentially in the listed order. For each candidate:

1. Add or strengthen characterization tests at the intended interface.
2. Move policy behind the deepened module.
3. Migrate callers.
4. Delete shallow modules, exports, and duplicate policy made unnecessary by the migration.
5. Run focused tests, static checks, the full test suite, and an affected-flow verification before starting the next candidate.

This sequencing keeps each regression attributable to one seam and protects recent Workspace Navigation and Incident Workbench fixes.

## Port Forward Lifecycle

`src/features/live-sessions/portForwardLifecycle.ts` becomes the single external module for querying, starting, restoring, reconnecting, and stopping port forwards.

It absorbs:

- Saved-forward reuse and local-port conflict policy from `saved-port-forward-actions.ts`.
- Restore eligibility and result summaries from `restore.ts`.
- Form-to-domain validation that currently requires `LiveSessionsSurface.svelte` to understand workspace scope.
- Query invalidation after lifecycle mutations.
- Saved-forward status persistence during lifecycle transitions.

`LiveSessionsSurface.svelte` retains presentation state, clipboard access, and lifecycle calls. Restore dismissal and auto-start-once tracking remain presentation state. The lifecycle module owns the pure eligibility decisions and accepts that state as input; it does not own state lifetime. The surface must not coordinate transport, persistence, invalidation, conflict detection, or the eligibility rules themselves.

Characterization tests must preserve dismissal per workspace and auto-start exactly once per workspace.

Tauri transport and workspace persistence remain accepted dependencies. No new public adapter abstraction is introduced because the repository does not have two adapters at either seam.

After migration, `saved-port-forward-actions.ts` and `restore.ts` are deleted if no caller remains. Helpers that encode Port Forward Lifecycle policy move into the deepened module or become private implementation. Display-only helpers remain local.

### Port-forward errors

Transport failures retain their current caller contract: operations that currently throw continue to throw, while saved-forward operations that currently return lifecycle results continue to return results. The deepened module owns saved-status updates and query invalidation ordering so callers cannot leave those steps inconsistent.

## Workspace Navigation

`src/app/svelte/workspaceNavigation.ts` becomes the source of truth for what navigation state means. Its implementation owns:

- Navigation intent reduction.
- Active workspace surface.
- Resource query scope.
- Display title and placeholder.
- Selected tree semantics.
- Cross-surface handoffs.
- Snapshot and restore behavior.

`workspaceShellModel.ts` retains sidebar-tree construction because it transforms fetched Kubernetes discovery data. It stops interpreting navigation state for resource query scope, title, placeholder, or active surface.

`WorkspaceShell.svelte` renders behavior derived by Workspace Navigation rather than repeating view-mode conditionals.

`tree-nav.ts` remains the tree vocabulary module. Scope resolution may remain an internal dependency, but navigation callers must not coordinate its mode flags.

Workspace Navigation transitions remain pure. Snapshot restoration first requires a matching workspace ID, preserving the current fallback. Workspace-ID matching is distinct from validating that restored nodes, resources, namespaces, and filters still belong to the workspace scope. Scope validation would be new behavior and is added only if a focused test proves a current inconsistency; characterization tests cover stale serialized state before any such correction.

Before policy moves, interface tests must preserve these query-safety invariants:

- Every non-resource surface produces `canQuery: false`.
- Namespace-list mode does not issue resource queries.
- An explicit namespace override takes precedence over tree-derived namespaces.
- Restored state for a different workspace falls back to defaults.

## Incident signal presentation

The backend Incident Workbench command remains the owner of Incident signal severity, signals, warning counts, and timestamps exposed through `IncidentCockpitItem`. Resource details retain their richer interpretation of raw conditions, events, container state, and logs. Changing that Tauri read-model seam or moving backend interpretation is outside this refactor.

A single module in `src/features/incidents/` deepens the presentation semantics that can be derived from the existing `IncidentCockpitItem` interface. Its implementation owns shared rules for:

- Incident Workbench ordering, filtering, and grouping.
- Explanatory summaries and labels.
- Detail pivots and next-action guidance.
- Selection and stable identity.

`src/features/resource-detail/incident-timeline.ts` remains the owner of timeline inclusion and tone because its input facts are not available through `IncidentCockpitItem`. Cross-surface behavior changes require characterization coverage that proves equivalent facts are represented by both interfaces before any shared rule is extracted.

Existing incident helper and model modules are consolidated only where consolidation reduces caller knowledge. Display-only formatting stays local when it does not encode Incident signal meaning. Unrelated resource-table and workspace behavior is not moved, and `src/lib/resource-health.ts` remains because it has active consumers outside incidents.

The presentation module accepts incomplete Incident Workbench data and preserves existing neutral fallbacks. It must not invent an Incident signal when facts are absent.

## Proving inconsistencies

A behavior correction is allowed only when a focused test or concrete reproduction demonstrates conflicting interpretation or lifecycle behavior. The proof must state the same input facts and the incompatible existing outputs. The corrected expectation becomes an interface test before production code changes.

No behavior changes are made solely because one implementation appears preferable.

## Verification

After each deepening:

1. Run focused tests for the changed interface and its callers.
2. Run TypeScript and Svelte static checks.
3. Run the full test suite.
4. Exercise the affected flow in the running application:
   - Port Forward Lifecycle: start, saved start, bulk restore, reconnect, and stop.
   - Workspace Navigation: move among resources, GitOps, Helm, incidents, settings, and live sessions; reload restored state.
   - Incident signals: compare the same signal in the Incident Workbench and resource details.
5. Review the diff for duplicate policy, leftover shallow modules, accidental new seams, and unrelated changes.

The implementation stops at the smallest deepening that passes these checks. It adds no speculative interface, dependency, configuration, or adapter type.
