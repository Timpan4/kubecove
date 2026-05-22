export interface ClusterContext {
	name: string;
	isCurrent: boolean;
}

export interface NamespaceSummary {
	name: string;
	age: string;
	createdAt?: string;
}

export interface ResourceSummary {
	kind: string;
	cluster: string;
	name: string;
	namespace: string | null;
	age: string;
	apiVersion?: string;
	group?: string;
	version?: string;
	plural?: string;
	namespaced?: boolean;
	dynamic?: boolean;
	createdAt?: string;
	status?: string;
	ready?: string;
	restarts?: number;
	ownerRef?: string;
	argoApp?: string;
	helmRelease?: string;
}

export type ResourceListRequest =
	| {
			kind: string;
			namespace?: string;
			resourceKind?: never;
	  }
	| {
			resourceKind: DiscoveredResourceKind;
			namespace?: string;
			kind?: never;
	  };

export interface ResourceDetails {
	kind: string;
	cluster: string;
	name: string;
	namespace: string | null;
	yaml: string;
}

export interface ResourceDetailsFull {
	summary: ResourceSummary;
	yaml: string;
	metadata: Record<string, unknown>;
	status?: Record<string, unknown>;
}

export type TopologyHealth =
	| "healthy"
	| "attention"
	| "degraded"
	| "restarted"
	| "unknown";

export type TopologyRelation = "owns" | "creates" | "groups";

export interface TopologyNode {
	id: string;
	kind: string;
	name: string;
	namespace: string | null;
	status?: string | null;
	health: TopologyHealth;
	selectable: boolean;
	summary: ResourceSummary;
}

export interface TopologyEdge {
	id: string;
	source: string;
	target: string;
	relation: TopologyRelation;
}

export interface ResourceTopology {
	nodes: TopologyNode[];
	edges: TopologyEdge[];
	warnings: string[];
}

export interface ResourceEventSummary {
	eventType: string;
	reason: string;
	message: string;
	count: number;
	lastSeen: string;
	lastSeenAt?: string;
	source: string;
	namespace: string | null;
}

export interface DiscoveredResourceKind {
	group: string;
	version: string;
	apiVersion: string;
	kind: string;
	plural: string;
	namespaced: boolean;
}

export interface WatchResourceKind {
	kind: string;
	group?: string;
	version?: string;
	apiVersion?: string;
	plural?: string;
	namespaced?: boolean;
}

export interface WatchResourceKey {
	resourceKind: WatchResourceKind;
	namespace?: string;
}

export interface PodLogStreamRequest {
	clusterContext: string;
	namespace: string;
	podName: string;
	container?: string;
	tailLines?: number;
}

export interface WatchResourceTarget {
	cluster: string;
	kind: string;
	namespace?: string | null;
	name?: string | null;
}

export interface AppUsageMetricsBreakdown {
	label: string;
	description: string;
	cpuPercent: number;
	memoryBytes: number;
	processCount: number;
	children: AppUsageMetricsBreakdown[];
}

export interface AppUsageMetrics {
	cpuPercent: number;
	memoryBytes: number;
	processCount: number;
	sampledAt: string;
	breakdown: AppUsageMetricsBreakdown[];
}

export type StreamStatus = "connected" | "reconnecting" | "stopped" | "error";

export type StreamMessage =
	| { type: "started"; streamId: string; label: string }
	| {
			type: "status";
			streamId: string;
			status: StreamStatus | string;
			message: string;
	  }
	| {
			type: "resourceChanged";
			streamId: string;
			target: WatchResourceTarget;
			action: string;
	  }
	| {
			type: "resourceEventsChanged";
			streamId: string;
			target: WatchResourceTarget;
			action: string;
	  }
	| { type: "logLine"; streamId: string; line: string }
	| { type: "error"; streamId: string; message: string }
	| { type: "stopped"; streamId: string };

export interface AppError {
	message: string;
	kind: string;
}

export const SUPPORTED_KINDS = [
	"Pod",
	"Deployment",
	"StatefulSet",
	"DaemonSet",
	"Service",
	"Ingress",
	"ConfigMap",
	"Secret",
	"PersistentVolumeClaim",
	"Job",
	"CronJob",
] as const;

// Cluster-scoped kinds - have no namespace
export const CLUSTER_SCOPED_KINDS = [
	"Node",
	"StorageClass",
	"PersistentVolume",
] as const;

export type SupportedKind = (typeof SUPPORTED_KINDS)[number];
export type ClusterScopedKind = (typeof CLUSTER_SCOPED_KINDS)[number];
export type AnyKind = SupportedKind | ClusterScopedKind;
export type ResourceKindSelection = AnyKind | DiscoveredResourceKind;

// Argo CD types - fields match Rust camelCase models exactly
// Optional fields are string | null per Rust Option<String> serialization
export interface ArgoApplicationSummary {
	name: string;
	cluster: string; // not clusterContext
	namespace: string | null; // null for cluster-scoped
	project: string | null;
	syncStatus: string | null;
	healthStatus: string | null;
	destinationNamespace: string | null;
	destinationServer: string | null;
	sourceRepo: string | null;
	sourceRevision: string | null;
	age: string;
	createdAt?: string;
}

export interface ArgoApplicationDetails {
	summary: ArgoApplicationSummary;
	yaml: string;
	metadata: Record<string, unknown>;
	status?: Record<string, unknown>; // backend returns Option, so optional
}

export interface ArgoApplicationSetSummary {
	name: string;
	cluster: string;
	namespace: string | null;
	age: string;
	createdAt?: string;
	project: string | null;
	status: string | null;
	syncStatus: string | null;
	healthStatus: string | null;
	destinationNamespace: string | null;
	destinationServer: string | null;
	sourceRepo: string | null;
	sourceRevision: string | null;
}

export interface ArgoApplicationSetDetails {
	summary: ArgoApplicationSetSummary;
	yaml: string;
	metadata: Record<string, unknown>;
}

export interface HelmReleaseSummary {
	cluster: string;
	name: string;
	namespace: string;
	age: string;
	updatedAt?: string;
	createdAt?: string;
	chart?: string;
	appVersion?: string;
	revision?: number;
	status?: string;
	storageKind: string;
	storageName: string;
}

export interface HelmReleaseDetails {
	summary: HelmReleaseSummary;
	yaml: string;
	metadata: Record<string, unknown>;
	release?: Record<string, unknown>;
}

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
	serviceAccounts: ServiceAccountSummary[];
	roles: RbacRoleSummary[];
	clusterRoles: RbacRoleSummary[];
	roleBindings: RbacBindingSummary[];
	clusterRoleBindings: RbacBindingSummary[];
	namespaceAccess: RbacNamespaceAccessSummary[];
}

export interface ArgoAppProjectSummary {
	name: string;
	cluster: string;
	namespace: string | null;
	age: string;
	createdAt?: string;
	description: string | null;
	status: string | null;
}

export interface ArgoAppProjectDetails {
	summary: ArgoAppProjectSummary;
	yaml: string;
	metadata: Record<string, unknown>;
}
