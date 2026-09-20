import type {
	ArgoApplicationInspector,
	ArgoOperationAction,
	JsonObject,
	JsonValue,
} from "@/lib/types";
import { messageFromError } from "@/lib/error-redaction";

export function argoValue(
	value: JsonValue | undefined,
	...path: string[]
): JsonValue | undefined {
	let current = value;
	for (const key of path) {
		if (!isRecord(current)) return undefined;
		current = current[key];
	}
	return current;
}

export function argoText(
	value: JsonValue | undefined,
	...path: string[]
): string | null {
	const result = argoValue(value, ...path);
	return String(result) === result ? result : null;
}

function isRecord(value: JsonValue | undefined): value is JsonObject {
	return value !== null && !Array.isArray(value) && Object(value) === value;
}

export function argoOperationProgress(inspector: ArgoApplicationInspector) {
	const state = inspector.operationState;
	const reported = argoText(state, "phase");
	const phase =
		inspector.operationRequested &&
		!["Running", "Terminating"].includes(reported ?? "")
			? "Pending"
			: reported;
	const resources = argoValue(state, "syncResult", "resources");
	return {
		phase,
		message: messageFromError(argoText(state, "message") ?? ""),
		active:
			inspector.operationRequested === true ||
			phase === "Running" ||
			phase === "Terminating",
		refreshing: Boolean(inspector.refreshRequested),
		resources: (Array.isArray(resources) ? resources : []).map((resource) => ({
			kind: argoText(resource, "kind") ?? "Resource",
			name: argoText(resource, "name") ?? "Unknown",
			namespace: argoText(resource, "namespace"),
			phase:
				argoText(resource, "hookPhase") ??
				argoText(resource, "status") ??
				"Unknown",
			message: messageFromError(argoText(resource, "message") ?? ""),
		})),
	};
}

export function argoObservation(inspector: ArgoApplicationInspector) {
	return {
		uid: inspector.application.uid,
		reconciledAt: argoText(inspector.status, "reconciledAt"),
		operation: argoText(inspector.operationState, "startedAt"),
	};
}

export function argoOperationOutcome(
	action: ArgoOperationAction,
	baseline: ReturnType<typeof argoObservation>,
	inspector: ArgoApplicationInspector,
	sawPending: boolean,
): "pending" | "succeeded" | "failed" {
	const current = argoObservation(inspector);
	if (action === "refresh" || action === "hardRefresh") {
		if (inspector.refreshRequested) return "pending";
		return sawPending ||
			(current.reconciledAt !== null &&
				current.reconciledAt !== baseline.reconciledAt)
			? "succeeded"
			: "pending";
	}
	const progress = argoOperationProgress(inspector);
	if (
		progress.active ||
		(!sawPending && current.operation === baseline.operation)
	)
		return "pending";
	if (progress.phase === "Succeeded") return "succeeded";
	if (progress.phase === "Failed" || progress.phase === "Error")
		return "failed";
	return "pending";
}
