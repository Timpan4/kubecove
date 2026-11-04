// Views captured by both visual drivers. The GPUI driver uses the same names.
export const VIEWPORT = { width: 1440, height: 900 } as const;
export const VIEWS = ["overview", "resources", "detail", "yaml", "events", "logs", "revisions", "actions", "cluster-overview", "incidents", "port-forwards", "rbac", "workspaces", "settings", "palette"] as const;
export type View = (typeof VIEWS)[number];
