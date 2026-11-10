import { detailMetadataFor, detailStatusFor, detailSummaryFor } from "./tauri-dev-mock-details";
import type { HealthAssessment, JsonObject, ResourceDetailsFull, ResourceSummary } from "./types";

// Detail-tab fixture for the Details parity scenarios (`e2e/parity/scenarios/detail-status.json`).
// Only one Pod carries richer metadata, conditions and evidence, so other mock details keep their shape.
const TARGET_NAME = "payments-api-7d9c9b7f8d-x4m8v";

const health: HealthAssessment = {
	state: "needsAttention",
	completeness: "complete",
	winningSources: ["containerRestart"],
	reasons: ["Restart count was 3 before the last rollout"],
	evidence: [
		{ source: "kubernetes", raw: "Running", state: "healthy", current: true, reason: "Containers are running" },
		{ source: "containerRestart", raw: 3, state: "needsAttention", current: false, reason: "Restart count was 3 before the last rollout" },
	],
};

export interface DetailStatusFixture {
	summary: ResourceSummary;
	metadata: JsonObject;
	status: JsonObject;
}

export function withDetailStatusFixture(details: ResourceDetailsFull): ResourceDetailsFull {
	details = { ...details, summary: detailSummaryFor(details.summary), metadata: detailMetadataFor(details.summary), status: { ...details.status, ...detailStatusFor(details.summary) } };
	const fixture = detailStatusFixture(details.summary);
	if (!fixture) return details;
	return { ...details, summary: fixture.summary, metadata: fixture.metadata, status: { ...details.status, ...fixture.status } };
}

function detailStatusFixture(summary: ResourceSummary): DetailStatusFixture | undefined {
	if (summary.kind !== "Pod" || summary.namespace !== "payments" || summary.name !== TARGET_NAME) return undefined;
	return {
		summary: { ...summary, healthAssessment: health },
		metadata: {
			name: summary.name,
			namespace: summary.namespace,
			uid: "3f1d9c52-0b7e-4c1a-9d8e-7a2b6c4e1f10",
			resourceVersion: "714382",
			creationTimestamp: "2026-06-29T09:12:00Z",
			deletionTimestamp: "2026-06-29T09:45:00Z",
			finalizers: ["example.io/payment-drain"],
			labels: { "app.kubernetes.io/name": summary.name },
			annotations: { "kubecove.dev/mock": "true" },
		},
		status: {
			conditions: [
				{ type: "PodScheduled", status: "True", lastTransitionTime: "2026-06-29T09:12:00Z" },
				{
					type: "ContainersReady",
					status: "False",
					reason: "ContainersNotReady",
					message: "Sidecar is still warming its cache.",
					lastTransitionTime: "2026-06-29T09:40:00Z",
				},
			],
			hostIP: "10.0.4.27",
			qosClass: "Burstable",
		},
	};
}
