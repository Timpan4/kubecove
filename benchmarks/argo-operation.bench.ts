import assert from "node:assert/strict";
import { bench, describe } from "vitest";
import {
	argoObservation,
	argoOperationOutcome,
	argoOperationProgress,
} from "@/features/gitops/argo-operation-progress";
import type { ArgoApplicationInspector } from "@/lib/types";

const resources = Array.from({ length: 250 }, (_, index) => ({
	kind: index % 2 === 0 ? "Deployment" : "Job",
	name: `resource-${index}`,
	namespace: `namespace-${index % 100}`,
	hookPhase: index % 17 === 0 ? "Failed" : undefined,
	status: index % 17 === 0 ? "Failed" : "Synced",
	message:
		index % 13 === 0
			? `authorization: Bearer token-${index} failed for resource ${index}`
			: `Resource ${index} reconciled successfully`,
}));

function inspector(phase: string): ArgoApplicationInspector {
	return {
		application: {
			name: "payments",
			namespace: "argocd",
			context: "prod/context-0",
			uid: "application-uid",
		},
		status: { reconciledAt: "2026-10-11T12:00:00Z" },
		history: [],
		resources: [],
		comparisons: [],
		conditions: [],
		operationState: {
			phase,
			startedAt: "2026-10-11T11:59:00Z",
			message: "client_secret=sample-token operation observed",
			syncResult: { resources },
		},
		connected: true,
		transport: "connected",
		provenance: "connected Argo inspection",
	};
}

const baselineInspector = inspector("Running");
const pendingInspector = { ...inspector("Running"), operationRequested: true };
const oldTerminalInspector = inspector("Succeeded");
const succeededInspector = inspector("Succeeded");
const failedInspector = inspector("Failed");
const baseline = argoObservation(baselineInspector);
const redactedMessage =
	argoOperationProgress(pendingInspector).resources[0]?.message ?? "";

assert.equal(resources.length, 250);
assert(!redactedMessage.includes("token-0"));
assert.equal(
	argoOperationOutcome("sync", baseline, succeededInspector, true),
	"succeeded",
);
assert.equal(
	argoOperationOutcome("sync", baseline, failedInspector, true),
	"failed",
);
assert.equal(
	argoOperationOutcome("sync", baseline, oldTerminalInspector, false),
	"pending",
);

describe("Argo operation observation F50", () => {
	// F25 redaction is exercised inside argoOperationProgress for operation and resource messages.
	bench(
		"F50 observation, resource/hook progress projection, outcome, and redaction",
		() => {
			for (const current of [
				pendingInspector,
				oldTerminalInspector,
				succeededInspector,
				failedInspector,
			]) {
				argoObservation(current);
				const progress = argoOperationProgress(current);
				argoOperationOutcome("sync", baseline, current, true);
				for (const resource of progress.resources) {
					void resource.name.length;
					void resource.message.length;
				}
			}
		},
	);
});
