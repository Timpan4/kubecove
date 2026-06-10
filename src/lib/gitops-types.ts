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
	age: string;
	createdAt?: string;
}

export interface ArgoApplicationDetails {
	summary: ArgoApplicationSummary;
	yaml: string;
	metadata: Record<string, unknown>;
	status?: Record<string, unknown>;
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
