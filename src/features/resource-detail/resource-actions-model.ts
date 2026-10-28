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

/** Failed or unconfirmed permission checks never claim access. */
export function actionAllowed(state: ResourceActionState | null, action: ResourceActionId): boolean {
	return permissionForAction(state, action)?.allowed === true;
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

/** The rollout row resumes a paused Deployment. */
export function actionForRow(row: ResourceActionRow, state: ResourceActionState | null): ResourceActionId {
	return row.id === "rollout" && state?.paused ? "resume" : row.action;
}

function entries(map: Record<string, string>): string {
	return Object.entries(map).map(([key, value]) => `${key} ${value}`).join(", ");
}

/** The row's current value in the loaded action state. */
export function currentRowLabel(row: ResourceActionRow, state: ResourceActionState | null, kind: string): string {
	if (!state) return "";
	const { containers } = state;
	switch (row.id) {
		case "replicas":
			return `${state.replicas ?? 0} desired`;
		case "rollout":
			return state.paused ? "Paused" : "Running";
		case "restart":
			return "Replace Pods gradually";
		case "rollback":
			return state.revisions.length ? `${state.revisions.length} available revisions` : "No owned revisions found";
		case "image":
			return containers.map((container) => `${container.name}: ${container.image}`).join(", ");
		case "environment":
			return containers
				.map((container) => `${container.name}: ${container.env.map((variable) => variable.name).join(", ") || "no variables"}`)
				.join("; ");
		case "resources":
			return containers
				.map((container) => `${container.name}: requests ${entries(container.requests) || "none"}; limits ${entries(container.limits) || "none"}`)
				.join("; ");
		case "labels":
		case "annotations":
			return (
				Object.entries(state[row.id]).map(([key, value]) => `${key}=${value}`).join(", ") ||
				(row.id === "labels" ? "No labels" : "No annotations")
			);
		case "delete":
			return `Deletes this ${kind} and its dependents where Kubernetes applies them.`;
	}
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
