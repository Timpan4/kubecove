import type {
	ClusterOperationPreview,
	ClusterOperationResult,
	DeleteResourceRequest,
	RolloutRestartRequest,
	ScaleWorkloadRequest,
} from "./types";

type MockArgs = Record<string, unknown> | undefined;

type OperationRequest =
	| DeleteResourceRequest
	| RolloutRestartRequest
	| ScaleWorkloadRequest;

export const operationMockHandlers = {
	preview_scale_workload: (args?: MockArgs) =>
		scaleOperationPreview(operationRequest<ScaleWorkloadRequest>(args)),
	scale_workload: (args?: MockArgs) =>
		scaleOperationResult(operationRequest<ScaleWorkloadRequest>(args)),
	preview_rollout_restart: (args?: MockArgs) =>
		restartOperationPreview(operationRequest<RolloutRestartRequest>(args)),
	rollout_restart: (args?: MockArgs) =>
		restartOperationResult(operationRequest<RolloutRestartRequest>(args)),
	preview_delete_resource: (args?: MockArgs) =>
		deleteOperationPreview(operationRequest<DeleteResourceRequest>(args)),
	delete_resource: (args?: MockArgs) =>
		deleteOperationResult(operationRequest<DeleteResourceRequest>(args)),
};

function operationRequest<T>(args?: MockArgs): T {
	const request = args?.request;
	if (!request || typeof request !== "object") {
		throw new Error("Operation request is required.");
	}
	return request as T;
}

function operationTarget(request: OperationRequest) {
	return {
		clusterContext: request.clusterContext,
		namespace: request.namespace,
		kind: request.kind,
		name: request.name,
	};
}

function operationLabel(request: OperationRequest) {
	return `${request.kind} ${request.namespace}/${request.name} in context ${request.clusterContext}`;
}

function requireMockConfirmation(request: OperationRequest) {
	if (!request.confirmed) throw new Error("Explicit confirmation is required.");
}

function scaleOperationPreview(
	request: ScaleWorkloadRequest,
): ClusterOperationPreview {
	return {
		target: operationTarget(request),
		effect: `Scale ${operationLabel(request)} to ${request.replicas} replicas`,
	};
}

function scaleOperationResult(request: ScaleWorkloadRequest): ClusterOperationResult {
	requireMockConfirmation(request);
	return {
		target: operationTarget(request),
		effect: `Simulated scaling ${operationLabel(request)} to ${request.replicas} replicas in browser mock mode.`,
	};
}

function restartOperationPreview(
	request: RolloutRestartRequest,
): ClusterOperationPreview {
	return {
		target: operationTarget(request),
		effect: `Restart ${operationLabel(request)} by updating its pod template`,
	};
}

function restartOperationResult(
	request: RolloutRestartRequest,
): ClusterOperationResult {
	requireMockConfirmation(request);
	return {
		target: operationTarget(request),
		effect: `Simulated restart of ${operationLabel(request)} in browser mock mode.`,
	};
}

function deleteOperationPreview(
	request: DeleteResourceRequest,
): ClusterOperationPreview {
	return {
		target: operationTarget(request),
		effect: `Delete ${operationLabel(request)}`,
	};
}

function deleteOperationResult(
	request: DeleteResourceRequest,
): ClusterOperationResult {
	requireMockConfirmation(request);
	return {
		target: operationTarget(request),
		effect: `Simulated deletion of ${operationLabel(request)} in browser mock mode.`,
	};
}
