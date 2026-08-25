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

export function guardedOperationBlocker(
	error: unknown,
): "permission" | "provider connection" | "operation support" {
	const bucket = friendlyErrorBucket(error);
	if (bucket === "forbiddenRbac") return "permission";
	if (
		bucket === "authentication" ||
		bucket === "kubeconfigConfig" ||
		bucket === "mixedWorkspaceConnection" ||
		bucket === "networkTransient"
	) return "provider connection";
	return "operation support";
}

export function guardedOperations(resource: ResourceSummary): GuardedOperations {
	const scope = (action: string) => `${action} this exact selected ${resource.kind} resource only.`;
	const available: GuardedOperation[] = [];
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
