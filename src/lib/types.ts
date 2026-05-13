export interface ClusterContext {
	name: string;
}

export interface NamespaceSummary {
	name: string;
	age: string;
}

export interface ResourceSummary {
	kind: string;
	cluster: string;
	name: string;
	namespace: string | null;
	age: string;
	status?: string;
	ready?: string;
	restarts?: number;
	ownerRef?: string;
	argoApp?: string;
	helmRelease?: string;
}

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

export interface ArgoAppProjectSummary {
	name: string;
	cluster: string;
	namespace: string | null;
	age: string;
	description: string | null;
	status: string | null;
}

export interface ArgoAppProjectDetails {
	summary: ArgoAppProjectSummary;
	yaml: string;
	metadata: Record<string, unknown>;
}
