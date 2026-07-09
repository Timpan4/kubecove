export interface ClusterContext {
	name: string;
	isCurrent: boolean;
}

export interface NamespaceSummary {
	name: string;
	age: string;
	createdAt?: string;
}

export interface GitOpsOwnerSummary {
	provider: "argo" | "flux" | string;
	kind: string;
	name: string;
	namespace?: string | null;
	confidence: "metadata" | "label" | "inventory" | string;
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
	health: ResourceHealth;
	createdAt?: string;
	status?: string;
	ready?: string;
	restarts?: number;
	ownerRef?: string;
	argoApp?: string;
	helmRelease?: string;
	gitOpsOwner?: GitOpsOwnerSummary;
	metrics?: ResourceMetricSummary;
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

export type {
	CancelBackendRequestsResult,
	CancellableRequest,
} from "./cancellation-types";

export type YamlViewMode = "kubectl" | "applyClean";
export type YamlEncoding = "yaml" | "kyaml";

export interface YamlApplyRequest {
	clusterContext: string;
	kubeconfigEnvVar?: string;
	kind: string;
	apiVersion?: string;
	group?: string;
	version?: string;
	plural?: string;
	namespaced?: boolean;
	name: string;
	namespace?: string | null;
	yaml: string;
	yamlEncoding?: YamlEncoding;
	forceConflicts?: boolean;
}

export interface YamlApplyTarget {
	clusterContext: string;
	kind: string;
	apiVersion?: string;
	name: string;
	namespace?: string | null;
}

export interface YamlApplyPreview {
	target: YamlApplyTarget;
	currentYaml: string;
	dryRunYaml: string;
}

export interface YamlApplyResult {
	target: YamlApplyTarget;
	appliedYaml: string;
}

export type KubernetesYamlLintSeverity = "error" | "warning" | "info";

export interface KubernetesYamlLintDiagnostic {
	severity: KubernetesYamlLintSeverity;
	source: string;
	message: string;
	fieldPath?: string;
}

export interface KubernetesYamlLintStatusNote {
	severity: KubernetesYamlLintSeverity;
	source: string;
	message: string;
}

export interface KubernetesYamlLintResult {
	diagnostics: KubernetesYamlLintDiagnostic[];
	notes: KubernetesYamlLintStatusNote[];
}

export type ResourceHealth =
	| "healthy"
	| "attention"
	| "degraded"
	| "restarted"
	| "unknown";
export type TopologyHealth = ResourceHealth;

export type TopologyMode = "ownership" | "networkFlow";
export type TopologyRelation =
	| "owns"
	| "creates"
	| "groups"
	| "routesTo"
	| "selects"
	| "targets";

export interface TopologyNode {
	id: string;
	kind: string;
	name: string;
	namespace: string | null;
	status?: string | null;
	health: TopologyHealth;
	portHints?: string[];
	metrics?: ResourceMetricSummary;
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

export type IncidentSeverity =
	| "degraded"
	| "attention"
	| "restarted"
	| "warning";

export interface IncidentSignalSummary {
	kind: string;
	label: string;
	message: string;
	source: string;
	lastSeenAt?: string;
}

export interface IncidentCockpitItem {
	resource: ResourceSummary;
	severity: IncidentSeverity;
	signals: IncidentSignalSummary[];
	warningEventCount: number;
	latestSignalAt?: string;
	latestWarningEvent?: ResourceEventSummary;
}

export interface IncidentCockpitSummary {
	cluster: string;
	generatedAt: string;
	requestedScope: ResourceListRequest[];
	items: IncidentCockpitItem[];
	warnings: string[];
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
	kubeconfigEnvVar?: string;
	namespace: string;
	podName: string;
	container?: string;
	tailLines?: number;
	sinceSeconds?: number;
}

export interface AggregatedLogStreamRequest {
	clusterContext: string;
	kubeconfigEnvVar?: string;
	namespace: string;
	targetKind: "Deployment" | "Service";
	targetName: string;
	tailLines?: number;
	sinceSeconds?: number;
}

export interface LogLineSource {
	podName: string;
	container?: string;
}

export interface PortForwardRequest {
	clusterContext: string;
	kubeconfigEnvVar?: string;
	namespace: string;
	targetKind?: "Pod" | "Service";
	targetName?: string;
	podName?: string;
	remotePort: number;
	localPort?: number;
}

export type PortForwardSessionStatus =
	| "listening"
	| "reconnecting"
	| "connected"
	| "error"
	| (string & {});

export interface PortForwardSessionSummary {
	id: string;
	clusterContext: string;
	kubeconfigEnvVar?: string;
	kubeconfigSourceKey?: string;
	kubeconfigSourceLabel?: string;
	namespace: string;
	targetKind: "Pod" | "Service" | string;
	targetName: string;
	podName: string;
	remotePort: number;
	resolvedPodName: string;
	resolvedPodPort: number;
	localPort: number;
	localAddress: string;
	localUrl: string;
	status: PortForwardSessionStatus;
	startedAt: string;
	lastError?: string;
}

export interface PodExecConfirmation {
	acknowledged: boolean;
	target: string;
	command: string;
}

export interface PodExecTerminalSize {
	cols: number;
	rows: number;
}

export interface PodExecSessionRequest {
	clusterContext: string;
	kubeconfigEnvVar?: string;
	namespace: string;
	podName: string;
	container?: string;
	command: string[];
	stdin: boolean;
	tty: boolean;
	terminalSize: PodExecTerminalSize;
	confirmation: PodExecConfirmation;
}

export type PodExecSessionStatus =
	| "starting"
	| "connecting"
	| "running"
	| "exited"
	| "error"
	| (string & {});

export interface PodExecSessionSummary {
	id: string;
	clusterContext: string;
	kubeconfigEnvVar?: string;
	kubeconfigSourceKey?: string;
	kubeconfigSourceLabel?: string;
	namespace: string;
	podName: string;
	container?: string;
	command: string[];
	stdin: boolean;
	tty: boolean;
	terminalCols: number;
	terminalRows: number;
	status: PodExecSessionStatus;
	startedAt: string;
	finishedAt?: string;
	exitCode?: number;
	lastError?: string;
}

export interface LiveSessionCleanupRequest {
	allowedClusterContexts: string[];
	kubeconfigSourceKey: string;
}

export interface LiveSessionCleanupResult {
	stoppedPortForwardIds: string[];
	stoppedPodExecIds: string[];
	stoppedPortForwards: number;
	stoppedPodExecSessions: number;
}

export interface KubeconfigPathEntry {
	path: string;
}

export interface KubeconfigSourceWarning {
	source: string;
	path?: string;
	message: string;
}

export interface KubeconfigSourcesSummary {
	kubeconfigEnvVar: string;
	paths: KubeconfigPathEntry[];
	sourceKey: string;
	sourceLabel: string;
	showSourceLabels: boolean;
	warnings: KubeconfigSourceWarning[];
}

export type {
	BackendDiagnosticEvent,
	BackendDiagnosticField,
	BackendDiagnosticStatus,
} from "./diagnostics-types";

export type PodExecSessionMessage =
	| {
			type: "started";
			sessionId: string;
			summary: PodExecSessionSummary;
	  }
	| {
			type: "status";
			sessionId: string;
			status: PodExecSessionStatus | string;
			message: string;
	  }
	| {
			type: "output";
			sessionId: string;
			stream: "stdout" | "stderr" | "terminal" | string;
			data: string;
	  }
	| { type: "error"; sessionId: string; message: string }
	| {
			type: "exited";
			sessionId: string;
			exitCode?: number;
			reason?: string;
			message?: string;
	  }
	| { type: "stopped"; sessionId: string };

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

export type ResourceMetricsAvailabilityStatus =
	| "available"
	| "unavailable"
	| "forbidden"
	| "noSamples";

export interface ResourceMetricsAvailability {
	status: ResourceMetricsAvailabilityStatus;
	message?: string;
}

export interface ResourceMetricSummary {
	kind: string;
	cluster: string;
	name: string;
	namespace: string | null;
	cpuMillicores?: number;
	memoryBytes?: number;
	sampledAt?: string;
	sourcePods: string[];
}

export interface ResourceMetricsSummary {
	cluster: string;
	availability: ResourceMetricsAvailability;
	pods: ResourceMetricSummary[];
	nodes: ResourceMetricSummary[];
	workloads: ResourceMetricSummary[];
	warnings: string[];
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
	| { type: "logLine"; streamId: string; line: string; source?: LogLineSource }
	| { type: "error"; streamId: string; message: string }
	| { type: "stopped"; streamId: string };

export interface AppError {
	message: string;
	kind: string;
}

export const SUPPORTED_KINDS = [
	"Pod",
	"Deployment",
	"ReplicaSet",
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
	"CustomResourceDefinition",
] as const;

export type SupportedKind = (typeof SUPPORTED_KINDS)[number];
export type ClusterScopedKind = (typeof CLUSTER_SCOPED_KINDS)[number];
export type AnyKind = SupportedKind | ClusterScopedKind;
export type ResourceKindSelection = AnyKind | DiscoveredResourceKind;

export type * from "./gitops-types";
export type * from "./helm-types";

export type * from "./rbac-types";
