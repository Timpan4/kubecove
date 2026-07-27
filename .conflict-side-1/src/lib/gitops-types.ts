export interface ArgoApplicationSourceSummary {
	repoUrl: string | null;
	targetRevision: string | null;
	resolvedRevision: string | null;
	path: string | null;
	chart: string | null;
	sourceMode?: "git" | "helm" | "plugin" | "unknown" | string | null;
	reference: string | null;
}

export interface ArgoApplicationSummary {
	name: string;
	cluster: string;
	namespace: string | null;
	project: string | null;
	syncStatus: string | null;
	healthStatus: string | null;
	destinationNamespace: string | null;
	destinationServer: string | null;
	sourceRepo: string | null;
	sourceRevision: string | null;
	sourceMode?: "git" | "helm" | "multi" | "plugin" | "unknown";
	sourceCount?: number;
	sources?: ArgoApplicationSourceSummary[];
	resourceNamespaces: string[];
	trackedResourceCount?: number;
	age: string;
	createdAt?: string;
	resourceVersion?: string | null;
	uid?: string | null;
}

export interface ArgoApplicationDetails {
	summary: ArgoApplicationSummary;
	yaml: string;
	metadata: Record<string, unknown>;
	status?: Record<string, unknown>;
}

export interface ArgoConnectionProfile {
	id: string;
	url: string;
	clusterContext?: string | null;
	workspaceId?: string | null;
	transport: "connected" | string;
	rememberCredential: boolean;
}

export interface ArgoConnectionStatus {
	profile: ArgoConnectionProfile | null;
	connected: boolean;
	username: string | null;
	unavailableReason: string | null;
}

export interface ArgoServerCapability {
	id: string;
	name: string;
	namespace: string | null;
	url: string | null;
	transport: string;
	unavailableReason: string | null;
}

export interface ArgoApplicationRef {
	name: string;
	namespace?: string | null;
	project?: string | null;
	resourceVersion?: string | null;
	uid?: string | null;
	apiVersion?: string | null;
	context?: string | null;
	workspaceId?: string | null;
}

export interface ArgoApplicationHistory {
	id?: number | null;
	revision?: string | null;
	revisions: string[];
	deployedAt?: string | null;
	initiatedBy?: string | null;
	source?: unknown;
	sources: unknown[];
}

export interface ArgoManagedResource {
	group?: string | null;
	version?: string | null;
	kind?: string | null;
	namespace?: string | null;
	name?: string | null;
	status?: string | null;
	health?: string | null;
	hook?: boolean | null;
	requiresPruning?: boolean | null;
	targetState?: unknown;
	liveState?: unknown;
}

export interface ArgoResourceComparison {
	resource: ArgoManagedResource;
	targetState?: unknown;
	liveState?: unknown;
	normalizedLiveState?: unknown;
	predictedLiveState?: unknown;
	modified?: boolean | null;
	exact?: boolean | null;
	provenance?: string | null;
	availableActions: ArgoResourceAction[];
}

export interface ArgoResourceActionParameter {
	name: string;
	required?: boolean;
	defaultValue?: string;
}

export interface ArgoResourceAction {
	name: string;
	disabled?: boolean;
	params?: ArgoResourceActionParameter[];
}

export interface ArgoApplicationInspector {
	application: ArgoApplicationRef;
	status?: unknown;
	history: ArgoApplicationHistory[];
	resources: ArgoManagedResource[];
	conditions: unknown[];
	operationState?: unknown;
	connected: boolean;
}

export type ArgoOperationAction =
	| "refresh"
	| "hardRefresh"
	| "sync"
	| "retry"
	| "rollback"
	| "terminate"
	| "resourceAction";

export interface ArgoOperationRequest {
	connectionId?: string | null;
	transport: "connected" | "kubernetes";
	application: ArgoApplicationRef;
	action: ArgoOperationAction;
	revision?: string | null;
	resources: ArgoManagedResource[];
	prune?: boolean | null;
	dryRun?: boolean | null;
	force?: boolean | null;
	historyId?: number | null;
	resourceAction?: string | null;
	resourceActionParameters?: unknown;
	resourceVersion?: string | null;
	clusterContext?: string | null;
	kubeconfigEnvVar?: string | null;
	preflightToken?: string | null;
	syncPayload?: unknown;
}

export interface ArgoOperationPreflight {
	allowed: boolean;
	transport: string;
	action: string;
	reason?: string | null;
	preflightToken?: string | null;
	resolvedRequest?: ArgoOperationRequest | null;
}

export interface ArgoOperationResult {
	accepted: boolean;
	transport: string;
	message: string;
	operation?: unknown;
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

export interface FluxResourceKind {
	group: string;
	version: string;
	apiVersion: string;
	kind: string;
	plural: string;
	namespaced: boolean;
	category: string;
}

export interface FluxDetectionSummary {
	detected: boolean;
	kinds: FluxResourceKind[];
	missingKinds: FluxResourceKind[];
}

export interface FluxInventoryResource {
	id: string;
	version?: string;
}

export interface FluxResourceSummary {
	cluster: string;
	name: string;
	namespace: string | null;
	age: string;
	createdAt?: string;
	resourceKind: FluxResourceKind;
	readyStatus?: string;
	suspended?: boolean;
	sourceKind?: string;
	sourceName?: string;
	sourceNamespace?: string;
	interval?: string;
	lastAppliedRevision?: string;
	message?: string;
	inventory: FluxInventoryResource[];
}

export interface FluxResourceDetails {
	summary: FluxResourceSummary;
	yaml: string;
	metadata: Record<string, unknown>;
	status?: Record<string, unknown>;
}
