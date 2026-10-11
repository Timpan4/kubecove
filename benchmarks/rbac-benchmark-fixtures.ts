import type {
	RbacBindingSummary,
	RbacInspectionSummary,
	RbacRiskIndicator,
	RbacRoleSummary,
	ServiceAccountSummary,
} from "@/lib/types";

function risks(index: number): RbacRiskIndicator[] {
	return index % 7 === 0
		? [{ level: "high", label: "Wildcard access", reason: "Synthetic risk" }]
		: index % 5 === 0
			? [{ level: "medium", label: "Broad verbs", reason: "Synthetic risk" }]
			: [];
}

function rules(index: number) {
	return [
		{
			verbs: index % 7 === 0 ? ["*"] : ["get", "list"],
			apiGroups: [""],
			resources: ["pods", "services"],
			resourceNames: [],
			nonResourceUrls: index % 10 === 0 ? ["/healthz"] : [],
			risks: risks(index),
		},
	];
}

export const rbacServiceAccounts: ServiceAccountSummary[] = Array.from(
	{ length: 500 },
	(_, index) => ({
		cluster: "prod",
		name: `account-${index}`,
		namespace: `namespace-${index % 100}`,
		age: "1m",
		automountToken: index % 3 !== 0,
		secretsCount: index % 3,
		imagePullSecretsCount: index % 2,
		risks: risks(index),
	}),
);

export const rbacRoles: RbacRoleSummary[] = Array.from(
	{ length: 500 },
	(_, index) => ({
		cluster: "prod",
		kind: "Role",
		name: `role-${index}`,
		namespace: `namespace-${index % 100}`,
		age: "1m",
		rulesCount: 1,
		risks: risks(index),
		rules: rules(index),
	}),
);

export const rbacClusterRoles: RbacRoleSummary[] = Array.from(
	{ length: 100 },
	(_, index) => ({
		cluster: "prod",
		kind: "ClusterRole",
		name: `cluster-role-${index}`,
		age: "1m",
		rulesCount: 1,
		risks: risks(index + 500),
		rules: rules(index + 500),
	}),
);

export const rbacRoleBindings: RbacBindingSummary[] = Array.from(
	{ length: 1_500 },
	(_, index) => {
		const accountIndex = index % rbacServiceAccounts.length;
		return {
			cluster: "prod",
			kind: "RoleBinding",
			name: `binding-${index}`,
			namespace: `namespace-${accountIndex % 100}`,
			age: "1m",
			roleRefKind: "Role",
			roleRefName: `role-${accountIndex}`,
			subjects:
				index % 17 === 0
					? [{ kind: "Group", name: "system:authenticated" }]
					: index % 19 === 0
						? [{ kind: "User", name: "operator", namespace: undefined }]
						: [
								{
									kind: "ServiceAccount",
									name: `account-${accountIndex}`,
									namespace: `namespace-${accountIndex % 100}`,
								},
							],
			risks: risks(index),
		};
	},
);

export const rbacClusterRoleBindings: RbacBindingSummary[] = Array.from(
	{ length: 300 },
	(_, index) => ({
		cluster: "prod",
		kind: "ClusterRoleBinding",
		name: `cluster-binding-${index}`,
		age: "1m",
		roleRefKind: "ClusterRole",
		roleRefName: `cluster-role-${index % rbacClusterRoles.length}`,
		subjects:
			index % 3 === 0
				? [{ kind: "Group", name: "system:authenticated" }]
				: [
						{
							kind: "ServiceAccount",
							name: `account-${index % 500}`,
							namespace: `namespace-${index % 100}`,
						},
					],
		risks: risks(index + 1_500),
	}),
);

export const rbacInspection: RbacInspectionSummary = {
	cluster: "prod",
	refreshedAt: "2026-10-11T12:00:00Z",
	warnings: [],
	coverage: [
		{
			family: "serviceAccounts",
			status: "complete",
			requestMode: "allNamespaces",
			count: 500,
		},
		{
			family: "roles",
			status: "complete",
			requestMode: "allNamespaces",
			count: 500,
		},
		{
			family: "clusterRoles",
			status: "complete",
			requestMode: "cluster",
			count: 100,
		},
		{
			family: "roleBindings",
			status: "complete",
			requestMode: "allNamespaces",
			count: 1_500,
		},
		{
			family: "clusterRoleBindings",
			status: "complete",
			requestMode: "cluster",
			count: 300,
		},
	],
	serviceAccounts: rbacServiceAccounts,
	roles: rbacRoles,
	clusterRoles: rbacClusterRoles,
	roleBindings: rbacRoleBindings,
	clusterRoleBindings: rbacClusterRoleBindings,
	namespaceAccess: Array.from({ length: 100 }, (_, index) => ({
		cluster: "prod",
		namespace: `namespace-${index}`,
		serviceAccounts: 5,
		roles: 5,
		roleBindings: 15,
		boundSubjects: [],
		risks: risks(index),
	})),
};
