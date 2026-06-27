# command-palette

Global ⌘K (macOS) / Ctrl+K (Windows, Linux) search-and-jump palette. Searches
static navigation targets (sidebar sections and curated kinds, Settings,
Workspaces), cluster namespaces, and resources from the cached resources
queries for the active cluster (warmed with the same query `ResourceBrowser`
runs, no dedicated backend command). Selection dispatches through the Svelte
workspace shell; open state lives in the Svelte command palette component.
