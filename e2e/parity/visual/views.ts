// Views captured by both visual drivers. The GPUI driver uses the same names.
export const VIEWPORT = { width: 1440, height: 900 } as const;
export const VIEWS = ["live-limited", "live-failed", "live-reconnecting", "custom-resources", "network-flow", "metrics", "usage-footer", "overview", "resources", "detail", "yaml", "events", "logs", "revisions", "actions", "actions-image", "actions-environment", "actions-resources", "actions-labels", "actions-annotations", "actions-rollback", "yaml-edit", "yaml-kyaml", "yaml-edit-kyaml", "cluster-overview", "incidents", "port-forwards", "rbac", "workspaces", "settings", "palette", "gitops", "gitops-list", "flux", "argo-app", "argo-workspace", "helm", "helm-list", "helm-release"] as const;
export type View = (typeof VIEWS)[number];
