# command-palette

Global ⌘K (macOS) / Ctrl+K (Windows, Linux) search-and-jump palette. Searches
static navigation targets (sidebar sections and curated kinds, Settings,
Workspaces), cluster namespaces, and resources from the cached resources
queries for the active cluster (warmed with the same query ResourceList runs —
no dedicated backend command). Selection dispatches through the
`useAppNavigation` handlers passed in from `App.tsx`; open state lives in a
small zustand store so the top-bar button and the global shortcut can toggle
it without prop threading.
