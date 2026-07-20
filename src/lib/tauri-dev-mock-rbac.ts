import type { RbacAccessReviewResult, RbacInspectionSummary } from "./types";
import { now } from "./tauri-dev-mock-data";

type MockArgs = Record<string, unknown> | undefined;

export function rbacInspectionMock(cluster = "mock-dev"): RbacInspectionSummary {
	const partial = cluster === "docker-desktop";
	return {
		cluster,
		refreshedAt: now,
		warnings: partial ? ["RoleBindings partially loaded in browser mock mode."] : [],
		coverage: [
			{ family: "serviceAccounts", status: "complete", requestMode: "allNamespaces", count: 3 },
			{ family: "roles", status: "complete", requestMode: "allNamespaces", count: 2 },
			{ family: "clusterRoles", status: "complete", requestMode: "cluster", count: 1 },
			{ family: "roleBindings", status: partial ? "partial" : "complete", requestMode: "allNamespaces", count: 2, message: partial ? "A later browser-mock page was unavailable." : undefined },
			{ family: "clusterRoleBindings", status: "complete", requestMode: "cluster", count: 1 },
		],
		serviceAccounts: [
			{ cluster, name: "payments-api", namespace: "payments", age: "18d", createdAt: "2026-06-11T08:00:00Z", automountToken: true, secretsCount: 1, imagePullSecretsCount: 0, risks: [{ level: "medium", label: "Mounted token", reason: "Service account token is mounted." }] },
			{ cluster, name: "grafana", namespace: "monitoring", age: "31d", createdAt: "2026-05-29T08:00:00Z", automountToken: false, secretsCount: 0, imagePullSecretsCount: 1, risks: [] },
			{ cluster, name: "argocd-application-controller", namespace: "argocd", age: "36d", createdAt: "2026-05-24T08:00:00Z", automountToken: true, secretsCount: 2, imagePullSecretsCount: 0, risks: [{ level: "high", label: "Broad app access", reason: "Mock controller can update application resources." }] },
		],
		roles: [
			{ cluster, kind: "Role", name: "payments-reader", namespace: "payments", age: "18d", createdAt: "2026-06-11T08:00:00Z", rulesCount: 1, risks: [], rules: [{ verbs: ["get", "list", "watch"], apiGroups: [""], resources: ["pods", "services", "configmaps"], resourceNames: [], nonResourceUrls: [], risks: [] }] },
			{ cluster, kind: "Role", name: "grafana-config", namespace: "monitoring", age: "31d", createdAt: "2026-05-29T08:00:00Z", rulesCount: 1, risks: [], rules: [{ verbs: ["get"], apiGroups: [""], resources: ["secrets", "configmaps"], resourceNames: ["grafana"], nonResourceUrls: [], risks: [] }] },
		],
		clusterRoles: [{ cluster, kind: "ClusterRole", name: "argocd-app-controller", age: "36d", createdAt: "2026-05-24T08:00:00Z", rulesCount: 2, risks: [{ level: "high", label: "Can update apps", reason: "Can update Argo CD Application resources." }], rules: [{ verbs: ["get", "list", "watch", "update"], apiGroups: ["argoproj.io"], resources: ["applications"], resourceNames: [], nonResourceUrls: [], risks: [{ level: "high", label: "Write access", reason: "Can update GitOps application state." }] }] }],
		roleBindings: [
			{ cluster, kind: "RoleBinding", name: "payments-api-reader", namespace: "payments", age: "18d", createdAt: "2026-06-11T08:00:00Z", roleRefKind: "Role", roleRefName: "payments-reader", subjects: [{ kind: "ServiceAccount", name: "payments-api", namespace: "payments" }], risks: [] },
			{ cluster, kind: "RoleBinding", name: "grafana-config", namespace: "monitoring", age: "31d", createdAt: "2026-05-29T08:00:00Z", roleRefKind: "Role", roleRefName: "grafana-config", subjects: [{ kind: "ServiceAccount", name: "grafana", namespace: "monitoring" }], risks: partial ? [{ level: "unknown", label: "Partial binding inventory", reason: "A later page was unavailable in this browser mock state." }] : [] },
		],
		clusterRoleBindings: [{ cluster, kind: "ClusterRoleBinding", name: "argocd-application-controller", age: "36d", createdAt: "2026-05-24T08:00:00Z", roleRefKind: "ClusterRole", roleRefName: "argocd-app-controller", subjects: [{ kind: "ServiceAccount", name: "argocd-application-controller", namespace: "argocd" }], risks: [{ level: "high", label: "Cluster-wide GitOps", reason: "Controller binding reaches all namespaces in mock data." }] }],
		namespaceAccess: [
			{ cluster, namespace: "payments", serviceAccounts: 1, roles: 1, roleBindings: 1, boundSubjects: [{ kind: "ServiceAccount", name: "payments-api", namespace: "payments" }], risks: [] },
			{ cluster, namespace: "monitoring", serviceAccounts: 1, roles: 1, roleBindings: 1, boundSubjects: [{ kind: "ServiceAccount", name: "grafana", namespace: "monitoring" }], risks: [] },
			{ cluster, namespace: "argocd", serviceAccounts: 1, roles: 0, roleBindings: 0, boundSubjects: [{ kind: "ServiceAccount", name: "argocd-application-controller", namespace: "argocd" }], risks: [{ level: "high", label: "ClusterRoleBinding", reason: "Bound to cluster-wide application-controller role." }] },
		],
	};
}

export function rbacReviewMock(args?: MockArgs): RbacAccessReviewResult {
	const request = args?.request as { target?: { verb?: string } } | undefined;
	const verb = request?.target?.verb?.trim().toLowerCase();
	if (verb === "impersonate") throw new Error("Browser mock verifier unavailable.");
	if (verb === "delete") return { outcome: "denied", reason: "Browser mock explicit denial." };
	if (verb === "watch") return { outcome: "noOpinion", reason: "Browser mock authorizer has no opinion." };
	return { outcome: "allowed", reason: "Browser mock explicit allowance." };
}
