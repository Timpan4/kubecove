import { friendlyErrorBucket } from "@/lib/friendly-errors";
import type { ResourceSummary } from "@/lib/types";

type ArgoTransport = "connected" | "kubernetes";
export type ArgoOperationBlocker = "permission" | "provider connection" | "operation support";

export function argoOperationTarget(
	resource: ResourceSummary,
	context: string,
	transport: ArgoTransport,
) {
	return {
		context,
		namespace: resource.namespace ?? "cluster-scoped",
		kind: resource.kind,
		resource: resource.name,
		operationScope: "Argo CD Application",
		transport: transport === "connected" ? "Connected Argo CD API" : "Kubernetes API",
	};
}

export function argoOperationAvailability({
	sourceReady,
	connectionReady,
	transport,
	connected,
	unavailableReason,
}: {
	sourceReady: boolean;
	connectionReady: boolean;
	transport: ArgoTransport;
	connected: boolean;
	unavailableReason?: string;
}) {
	if (!sourceReady) {
		return {
			available: false,
			blocker: "provider connection" as const,
			reason: "Selected Kubernetes context is unavailable.",
		};
	}
	if (!connectionReady || (transport === "connected" && !connected)) {
		return {
			available: false,
			blocker: "provider connection" as const,
			reason: unavailableReason ?? "Selected Argo CD provider connection is unavailable.",
		};
	}
	return {
		available: true,
		blocker: null,
		reason: transport === "connected"
			? "Available. Connected Argo CD authorization is checked during operation review."
			: "Available. Kubernetes permission is checked during operation review.",
	};
}

export function argoOperationBlocker(error: unknown): ArgoOperationBlocker {
	const bucket = friendlyErrorBucket(error);
	if (bucket === "forbiddenRbac") return "permission";
	if (
		bucket === "authentication" ||
		bucket === "kubeconfigConfig" ||
		bucket === "mixedWorkspaceConnection" ||
		bucket === "networkTransient" ||
		bucket === "providerDiscoveryUnavailable"
	) return "provider connection";
	return "operation support";
}
