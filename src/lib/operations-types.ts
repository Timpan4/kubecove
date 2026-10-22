export interface ClusterOperationTarget {
	clusterContext: string;
	kind: string;
	name: string;
	apiVersion?: string;
	namespace?: string | null;
}

export interface ClusterOperationResult {
	target: ClusterOperationTarget;
	effect: string;
}

export type ResourceActionKind = "edit" | "restart" | "pause" | "resume" | "rollback" | "delete";

export interface ResourceActionRequest extends ClusterOperationTarget {
	action: ResourceActionKind;
	edits?: ResourceActionEdits;
	revision?: number;
	expectedUid?: string;
	expectedResourceVersion?: string;
	expectedRevisionUid?: string;
	expectedRevisionResourceVersion?: string;
	confirmed: boolean;
	kubeconfigEnvVar?: string;
}

export interface ResourceActionStateRequest extends ClusterOperationTarget {
	kubeconfigEnvVar?: string;
}

export interface ResourceActionEdits {
	replicas?: number;
	containers?: ResourceContainerEdit[];
	labels?: Record<string, string | null>;
	annotations?: Record<string, string | null>;
}

export interface ResourceContainerEdit {
	name: string;
	image?: string;
	env?: ResourceEnvEdit[];
	requests?: Record<string, string | null>;
	limits?: Record<string, string | null>;
}

export interface ResourceEnvEdit {
	name: string;
	value?: string;
	remove?: boolean;
}

export interface ResourceActionContainer {
	name: string;
	image: string;
	env: { name: string; value?: string; referenced: boolean }[];
	requests: Record<string, string>;
	limits: Record<string, string>;
}

export interface ResourceActionPermission {
	action: ResourceActionKind;
	allowed: boolean;
	reason: string;
	requiredPermission: string;
}

export interface ResourceActionRevision {
	revision: number;
	images: string[];
}

export interface ResourceActionState {
	target: ClusterOperationTarget;
	uid: string;
	resourceVersion: string;
	replicas?: number;
	paused?: boolean;
	containers: ResourceActionContainer[];
	labels: Record<string, string>;
	annotations: Record<string, string>;
	permissions: ResourceActionPermission[];
	revisions: ResourceActionRevision[];
}

export interface ResourceActionPreview {
	target: ClusterOperationTarget;
	effect: string;
	beforeYaml: string;
	afterYaml: string;
	uid: string;
	resourceVersion: string;
	revisionUid?: string;
	revisionResourceVersion?: string;
}
