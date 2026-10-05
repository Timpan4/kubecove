import { friendlyErrorBucket } from "@/lib/friendly-errors";
import type { ResourceSummary } from "@/lib/types";

export type GuardedOperationId = "scale" | "restart" | "delete";

export interface GuardedOperation {
	id: GuardedOperationId;
	label: string;
	previewLabel: string;
	executeLabel: string;
	scope: string;
	destructive?: boolean;
	requiresReplicas?: boolean;
}

export interface GuardedOperations {
	available: GuardedOperation[];
	blocker: string | null;
}

export function guardedOperationBlocker<Cause>(
	cause: Cause,
): "permission" | "provider connection" | "operation support" {
	const bucket = friendlyErrorBucket(cause);
	if (bucket === "forbiddenRbac") return "permission";
	if (
		bucket === "authentication" ||
		bucket === "kubeconfigConfig" ||
		bucket === "mixedWorkspaceConnection" ||
		bucket === "networkTransient"
	) return "provider connection";
	return "operation support";
}

const BUILTIN_API_VERSION = new Map([
	["Deployment", "apps/v1"],
	["StatefulSet", "apps/v1"],
	["DaemonSet", "apps/v1"],
	["Pod", "v1"],
	["ConfigMap", "v1"],
]);

/** True only for the built-in resource the backend operates on, not a CRD sharing its kind. */
export function isBuiltinOperationTarget(resource: ResourceSummary): boolean {
	return (
		!resource.dynamic &&
		(resource.apiVersion === undefined || resource.apiVersion === BUILTIN_API_VERSION.get(resource.kind))
	);
}

export function guardedOperations(resource: ResourceSummary): GuardedOperations {
	const scope = (action: string) => `${action} this exact selected ${resource.kind} resource only.`;
	const available: GuardedOperation[] = [];
	if (!isBuiltinOperationTarget(resource)) {
		return { available, blocker: `Blocker: ${resource.kind} resource is not a built-in Kubernetes workload or core resource.` };
	}
	if (resource.kind === "Deployment" || resource.kind === "StatefulSet") {
		available.push(
			{
				id: "scale",
				label: "Scale workload",
				previewLabel: "Preview scale",
				executeLabel: "Scale workload",
				scope: scope("Scale replicas of"),
				requiresReplicas: true,
			},
			{
				id: "restart",
				label: "Rollout restart",
				previewLabel: "Preview restart",
				executeLabel: "Rollout restart",
				scope: scope("Restart"),
			},
		);
	} else if (resource.kind === "DaemonSet") {
		available.push({
			id: "restart",
			label: "Rollout restart",
			previewLabel: "Preview restart",
			executeLabel: "Rollout restart",
			scope: scope("Restart"),
		});
	} else if (resource.kind === "Pod" || resource.kind === "ConfigMap") {
		available.push({
			id: "delete",
			label: "Delete resource",
			previewLabel: "Preview delete",
			executeLabel: "Delete resource",
			scope: scope("Delete"),
			destructive: true,
		});
	}
	return {
		available,
		blocker: available.length === 0
			? `Blocker: resource kind ${resource.kind} has no supported guarded operation.`
			: null,
	};
}
