import type { ResourceActionRequest, ResourceActionState, ResourceSummary } from "@/lib/types";

const BUILTIN_API_VERSION = new Map([
	["Deployment", "apps/v1"],
	["StatefulSet", "apps/v1"],
	["DaemonSet", "apps/v1"],
	["Pod", "v1"],
	["ConfigMap", "v1"],
]);

/** True only for a built-in resource the backend operates on, not a CRD sharing its kind. */
export function isBuiltinOperationTarget(resource: ResourceSummary): boolean {
	return (
		!resource.dynamic &&
		(resource.apiVersion === undefined || resource.apiVersion === BUILTIN_API_VERSION.get(resource.kind))
	);
}

export type ResourceActionId = ResourceActionRequest["action"];
export type ResourceActionRowId =
	| "replicas"
	| "rollout"
	| "restart"
	| "rollback"
	| "image"
	| "environment"
	| "resources"
	| "labels"
	| "annotations"
	| "delete";

export interface ResourceActionRow {
	id: ResourceActionRowId;
	label: string;
	group: string;
	search: string;
	action: ResourceActionId;
}

const WORKLOAD_EDIT_ROWS: ResourceActionRow[] = [
	{ id: "image", label: "Image", group: "Container", search: "image container", action: "edit" },
	{ id: "environment", label: "Environment", group: "Container", search: "environment env variables", action: "edit" },
	{ id: "resources", label: "Requests and limits", group: "Container", search: "resources cpu memory requests limits", action: "edit" },
];

const METADATA_ROWS: ResourceActionRow[] = [
	{ id: "labels", label: "Labels", group: "Metadata", search: "labels metadata", action: "edit" },
	{ id: "annotations", label: "Annotations", group: "Metadata", search: "annotations metadata", action: "edit" },
];

const DELETE_ROW: ResourceActionRow = {
	id: "delete",
	label: "Delete resource",
	group: "",
	search: "delete remove",
	action: "delete",
};

/** Rows the ADR allows for this built-in resource kind. */
export function resourceActionRows(resource: ResourceSummary): ResourceActionRow[] {
	if (!isBuiltinOperationTarget(resource) || resource.kind === "Secret") return [];
	const rows: ResourceActionRow[] = [];
	if (resource.kind === "Deployment" || resource.kind === "StatefulSet") {
		rows.push({ id: "replicas", label: "Replicas", group: "Workload", search: "scale replicas", action: "edit" });
	}
	if (resource.kind === "Deployment") {
		rows.push({ id: "rollout", label: "Rollout", group: "Workload", search: "pause resume rollout", action: "pause" });
	}
	if (resource.kind === "Deployment" || resource.kind === "StatefulSet" || resource.kind === "DaemonSet") {
		rows.push({ id: "restart", label: "Restart", group: "Workload", search: "restart rollout", action: "restart" });
		if (resource.kind === "Deployment") {
			rows.push({ id: "rollback", label: "Revision", group: "Workload", search: "rollback revision history", action: "rollback" });
		}
		rows.push(...WORKLOAD_EDIT_ROWS);
	}
	if (["Deployment", "StatefulSet", "DaemonSet", "Pod", "ConfigMap"].includes(resource.kind)) {
		rows.push(...METADATA_ROWS, DELETE_ROW);
	}
	return rows;
}

export function permissionForAction(
	state: ResourceActionState | null,
	action: ResourceActionId,
) {
	return state?.permissions.find((permission) => permission.action === action);
}

export function fallbackPermission(action: ResourceActionId, kind: string): string {
	const plural = new Map([
		["Deployment", "deployments"],
		["StatefulSet", "statefulsets"],
		["DaemonSet", "daemonsets"],
		["Pod", "pods"],
		["ConfigMap", "configmaps"],
	]);
	const resource = plural.get(kind) ?? "resources";
	if (action === "delete") return `delete ${resource}`;
	return `patch ${resource}`;
}

export function hasPendingResourceActionEdits(edits: {
	replicas?: number;
	containers?: Array<{
		image?: string;
		env?: unknown[];
		requests?: Record<string, string | null>;
		limits?: Record<string, string | null>;
	}>;
	labels?: Record<string, string | null>;
	annotations?: Record<string, string | null>;
}): boolean {
	return Boolean(
		edits.replicas !== undefined ||
		edits.containers?.some((container) =>
			container.image !== undefined ||
			Boolean(container.env?.length) ||
			Boolean(Object.keys(container.requests ?? {}).length) ||
			Boolean(Object.keys(container.limits ?? {}).length),
		) ||
		Object.keys(edits.labels ?? {}).length ||
		Object.keys(edits.annotations ?? {}).length,
	);
}

export function resourceActionEditCount(edits: {
	replicas?: number;
	containers?: Array<{
		image?: string;
		env?: unknown[];
		requests?: Record<string, string | null>;
		limits?: Record<string, string | null>;
	}>;
	labels?: Record<string, string | null>;
	annotations?: Record<string, string | null>;
}): number {
	return (
		(edits.replicas === undefined ? 0 : 1) +
		(edits.containers?.reduce(
			(total, container) =>
				total +
				(container.image === undefined ? 0 : 1) +
				(container.env?.length ?? 0) +
				Object.keys(container.requests ?? {}).length +
				Object.keys(container.limits ?? {}).length,
			0,
		) ?? 0) +
		Object.keys(edits.labels ?? {}).length +
		Object.keys(edits.annotations ?? {}).length
	);
}

export function actionTarget(resource: ResourceSummary) {
	return {
		clusterContext: resource.cluster,
		namespace: resource.namespace,
		kind: resource.kind,
		name: resource.name,
		apiVersion: resource.apiVersion,
	};
}
