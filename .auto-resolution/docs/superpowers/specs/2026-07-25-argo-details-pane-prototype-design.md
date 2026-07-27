# Argo CD Details-Pane Prototype Design

## Goal

Replace the current partial Argo CD details-pane comparison with three complete, structurally different prototypes. Each prototype redesigns the entire content of the **Argo CD** tab inside the existing resource details pane, including the lower Overview, managed-resource, history, and diff experiences.

The design question is: which KubeCove-native information hierarchy best supports understanding Application state, inspecting drift, and starting familiar Argo operations from a constrained details pane?

## Scope Boundary

This work is frontend-only prototype exploration.

- Use deterministic fixture data and Application-scoped in-memory state.
- Simulate Refresh, Hard refresh, default Sync, advanced Sync, progress, confirmation, and completion feedback locally.
- Preserve the outer resource-detail navigation: Details, Events, Argo CD, Actions, and YAML.
- Preserve the independent resource-overview prototype and its selector.
- Do not change Rust, Tauri commands, backend contracts, authorization, production queries, watches, persistence, or real Argo operations.
- Do not turn the selected direction into production implementation during this prototype.

## Shared Prototype Model

The three versions use the same Application, revisions, conditions, managed resources, history, manifests, phase transitions, and sync defaults so their visual comparison is meaningful.

Only these concerns are shared across versions:

- Deterministic data and derived counts.
- The details-pane direction selector.
- Local Refresh and Sync state transitions.
- Advanced-sync confirmation rules.
- Toast feedback.

The full pane presentation is not shared. Each version owns its navigation, information hierarchy, and rendering of all content below the selector.

## Direction 1: Mission Control

A status-first, single-scroll operational dashboard.

- Application health, sync status, current operation, and Refresh/Sync controls form the opening region.
- Conditions and reconciliation counts appear immediately after the status region.
- Affected resources are prioritized ahead of healthy resources.
- Delivery source, destination, and revision are concise supporting context.
- Recent deployment history is integrated into the page rather than isolated behind a generic tab.
- Diff opens from an affected resource or change summary in an anchored in-pane inspection region.

This direction optimizes for answering: **What is wrong, and what needs attention now?**

## Direction 2: Resource Workbench

A resource-first master/detail inspector.

- A compact Application status and operation bar remains visible at the top.
- Managed resources form the primary navigation surface.
- Selecting a resource shows health, sync state, target/live metadata, and manifest comparison alongside or directly below the list depending on pane width.
- Filters distinguish drifted, unhealthy, hook, prunable, and healthy resources.
- Revision history becomes contextual navigation for comparing the selected resource across deployments.
- Application-level conditions remain visible without displacing the selected-resource workflow.

This direction optimizes for answering: **Which resource is causing the Application state, and how does it differ?**

## Direction 3: Change Review

A sync- and drift-focused review flow.

- The opening region summarizes the proposed reconciliation: revision, changed resources, prune candidates, and risk-sensitive sync settings.
- Changed and prunable resources are grouped as the primary content; healthy resources are collapsed into supporting context.
- Selecting a change opens its target-versus-live diff in the same workflow.
- History provides revision comparison context and makes the currently deployed revision explicit.
- Refresh and Sync split controls remain immediately available, with advanced choices anchored to Sync.
- Local confirmation appears only when options exceed the Application defaults.

This direction optimizes for answering: **What will change if I sync, and is it safe to proceed?**

## Interaction and Feedback

- Main Refresh performs the local normal-refresh simulation; its chevron exposes Hard refresh.
- Main Sync uses Application defaults; its chevron exposes revision override, prune, dry run, and force/replace.
- Switching direction changes only the details-pane prototype and preserves current local Application state.
- Pressed, busy, completion, warning, and error states appear immediately and consistently.
- Popovers remain anchored to their trigger. Motion is restrained, interruptible where relevant, and reduced-motion safe.
- Toasts announce simulated completion without implying that a real cluster operation occurred.

## Responsive Behavior

- At wide detail-pane widths, Workbench may use a master/detail split and the other versions may use structured multi-column regions.
- At narrow or full-screen detail widths, every version becomes a single readable flow without horizontal dependence.
- Direction selectors and primary actions remain visible and keyboard accessible.
- Long repository URLs, revisions, resource names, and manifest content truncate or scroll without breaking surrounding layout.

## Accessibility

- Direction and view navigation use named controls with visible selected states.
- Status is communicated through text as well as color.
- Progress and toast feedback use appropriate live regions.
- Dialogs and popovers preserve focus behavior supplied by existing KubeCove primitives.
- Dense resource and diff views retain readable focus, hover, and selected states.

## Verification

- Run TypeScript, Svelte, focused prototype tests, production build, and diff whitespace checks.
- Render all three complete versions at desktop and narrow detail-pane widths.
- Exercise direction switching, resource selection, history/revision selection, diff viewing, Refresh, Hard refresh, default Sync, and advanced Sync confirmation.
- Confirm the separate resource-overview selector remains unchanged.
- Instrument browser invocations and verify the prototype triggers no production Argo operation, inspector, comparison, connection, or watch calls.
