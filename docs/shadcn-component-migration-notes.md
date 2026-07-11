# ShadCN Svelte Component Audit (2026-06-29)

This is a quick pass over the frontend for places that still use native form/table controls or action markup where ShadCN Svelte components are available.

## Completed high-confidence migrations

- Done: `src/app/svelte/SimpleTable.svelte` now composes the shared shadcn Svelte table primitives, so downstream diagnostics, Helm, and RBAC simple tables inherit the component styling.

- Done: `src/app/svelte/SidebarTreeNode.svelte` now uses the shared shadcn Svelte `Button` primitive for the expand/collapse control.

- Done: `src/app/svelte/AppUsageFooter.svelte` now uses the shared shadcn Svelte table primitives for process rows.

- Done: `src/app/svelte/HelmSurface.svelte` now uses the shared shadcn Svelte table primitives for release and reconciliation tables.

- Done: `src/app/svelte/IncidentSurface.svelte` now uses the shared shadcn Svelte table primitives for the incident list.

- Done: `src/features/live-sessions/LiveSessionsSurface.svelte` now uses the shared shadcn Svelte table primitives for active port-forward, saved forward, and exec-session tables, plus `Field` + `FieldLabel` for the auto-start checkbox.

## Completed medium-priority cleanup

- Done: `src/app/svelte/DiagnosticsSettings.svelte` now uses shadcn Svelte `Field` + `FieldLabel` for the diagnostics report checkbox.

- Done: `src/app/svelte/WorkspaceShell.svelte` now uses shadcn Svelte `Button` for the search trigger and `Field` + `FieldLabel` for the auto-start checkbox.

- Done: `src/features/resources/ResourceBrowserTopBar.svelte` now uses shadcn Svelte `Button` for the health summary filter controls.

- Done: `src/features/resources/ResourceBrowser.svelte` now uses shadcn Svelte `Button` for the map/table rail controls, grouped-section toggles, and sortable header controls.

## Not considered

- `src/features/workspaces/WorkspaceLauncher.svelte` contains `<form class="contents">` for layout; this is low-risk and not necessarily a shadcn migration item unless you want a `Form` wrapper pattern applied across the app.

- `src/components/YamlCodeEditor.svelte` and topology spike entry points are intentionally not form-style UI surfaces and are excluded from this list.
