export const gitRepositoryUrl = "git://git-server.e2e-system.svc.cluster.local:9418/fixtures.git";

export const platformApplicationNames = ["platform-argocd", "platform-cilium", "platform-metrics", "platform-storage", "platform-ingress"] as const;
export const tenantApplicationNames = ["tenant-catalog", "tenant-ledger"] as const;

export const bootstrapOrder = ["kind", "cilium", "git", "argocd", "root-application", "platform-applications", "tenant-applications", "operations-helm-release"] as const;

export function readinessPhase(resource: string, condition: string) {
	if (resource === "node" || resource === "deployment") return condition === "Ready" ? "ready" : "waiting";
	if (resource === "application") return condition === "Healthy" || condition === "Synced" ? "ready" : "waiting";
	return "unknown";
}

