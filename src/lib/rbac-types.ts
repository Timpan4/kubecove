export type RbacRiskLevel = "low" | "medium" | "high" | "unknown";
export type RbacCoverageStatus = "complete" | "partial" | "unavailable";
export type RbacFamily = "serviceAccounts" | "roles" | "clusterRoles" | "roleBindings" | "clusterRoleBindings";
export type RbacRequestMode = "allNamespaces" | "cluster";

export interface RbacCoverage {
	family: RbacFamily;
	status: RbacCoverageStatus;
	requestMode: RbacRequestMode;
	count: number;
	namespaces?: string[];
	message?: string;
}

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
	refreshedAt?: string;
	warnings: string[];
	coverage: RbacCoverage[];
	serviceAccounts: ServiceAccountSummary[];
	roles: RbacRoleSummary[];
	clusterRoles: RbacRoleSummary[];
	roleBindings: RbacBindingSummary[];
	clusterRoleBindings: RbacBindingSummary[];
	namespaceAccess: RbacNamespaceAccessSummary[];
}

export type RbacAccessReviewIdentity =
	| { kind: "serviceAccount"; name: string; namespace: string }
	| { kind: "user"; username: string; groups: string[] }
	| { kind: "group"; group: string };

export type RbacAccessReviewTarget =
	| {
		kind: "resource";
		verb: string;
		resource: string;
		apiGroup?: string;
		namespace: string | null;
		subresource?: string;
		name?: string;
	}
	| { kind: "nonResource"; verb: string; nonResourceUrl: string };

export interface RbacAccessReviewRequest {
	clusterContext: string;
	identity: RbacAccessReviewIdentity;
	target: RbacAccessReviewTarget;
	kubeconfigEnvVar?: string;
	requestId?: string;
	cancelScope?: string;
}

export interface RbacAccessReviewResult {
	outcome: "allowed" | "denied" | "noOpinion";
	reason?: string;
	evaluationError?: string;
}
