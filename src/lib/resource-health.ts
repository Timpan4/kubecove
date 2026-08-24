import type {
	HealthAssessment,
	HealthAssessmentSource,
	HealthAssessmentState,
	ResourceSummary,
} from "./types";

export interface ResourceHealthFlags {
	healthy: boolean;
	attention: boolean;
	degraded: boolean;
	unknown: boolean;
	notEvaluated: boolean;
	restarted: boolean;
}

export function resourceHealthAssessment(row: ResourceSummary): HealthAssessment | null | undefined {
	return row.healthAssessment;
}

export function classifyResourceHealth(row: ResourceSummary): ResourceHealthFlags {
	const state = resourceHealthAssessment(row)?.state;
	return {
		healthy: state === "healthy",
		attention: state === "needsAttention",
		degraded: state === "degraded",
		unknown: state === undefined || state === "unknown",
		notEvaluated: state === "notEvaluated",
		restarted: (row.restarts ?? 0) > 0,
	};
}

export function healthStateLabel(state: HealthAssessmentState): string {
	if (state === "needsAttention") return "Needs attention";
	if (state === "notEvaluated") return "Not evaluated";
	return state[0].toUpperCase() + state.slice(1);
}

export function healthSourceLabel(source: HealthAssessmentSource): string {
	const labels: Record<HealthAssessmentSource, string> = {
		kubernetes: "Kubernetes status/conditions",
		argoHealth: "Argo CD health",
		argoSync: "Argo CD sync",
		warningEvent: "Kubernetes Warning Event",
		containerRestart: "Container restart",
		provider: "Provider availability",
		healthContract: "Health contract",
	};
	return labels[source];
}

export function healthSourceSummary(assessment: HealthAssessment): string {
	return assessment.winningSources.length > 0
		? assessment.winningSources.map(healthSourceLabel).join(" + ")
		: "Health contract";
}
