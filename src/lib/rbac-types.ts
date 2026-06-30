export type RbacRiskLevel = "low" | "medium" | "high";

export interface RbacRiskIndicator {
	level: RbacRiskLevel;
	label: string;
	reason: string;
}

export interface RbacRuleSummary {
	verbs: string[];
	apiGroups: string[];
	resources: string[];
	resourceNames: string[];
	nonResourceUrls: string[];
	risks: RbacRiskIndicator[];
}

export interface RbacSubjectSummary {
	kind: string;
	name: string;
	namespace?: string;
}

export interface ServiceAccountSummary {
	cluster: string;
	name: string;
	namespace: string;
	age: string;
	createdAt?: string;
	automountToken?: boolean;
	secretsCount: number;
	imagePullSecretsCount: number;
	risks: RbacRiskIndicator[];
}

export interface RbacRoleSummary {
	cluster: string;
	kind: "Role" | "ClusterRole" | string;
	name: string;
	namespace?: string;
	age: string;
	createdAt?: string;
	rulesCount: number;
	risks: RbacRiskIndicator[];
	rules: RbacRuleSummary[];
}

export interface RbacBindingSummary {
	cluster: string;
	kind: "RoleBinding" | "ClusterRoleBinding" | string;
	name: string;
	namespace?: string;
	age: string;
	createdAt?: string;
	roleRefKind: string;
	roleRefName: string;
	subjects: RbacSubjectSummary[];
	risks: RbacRiskIndicator[];
}

export interface RbacNamespaceAccessSummary {
	cluster: string;
	namespace: string;
	serviceAccounts: number;
	roles: number;
	roleBindings: number;
	boundSubjects: RbacSubjectSummary[];
	risks: RbacRiskIndicator[];
}

export interface RbacInspectionSummary {
	cluster: string;
	warnings: string[];
	serviceAccounts: ServiceAccountSummary[];
	roles: RbacRoleSummary[];
	clusterRoles: RbacRoleSummary[];
	roleBindings: RbacBindingSummary[];
	clusterRoleBindings: RbacBindingSummary[];
	namespaceAccess: RbacNamespaceAccessSummary[];
}
