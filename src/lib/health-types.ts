export type HealthAssessmentState =
	| "healthy"
	| "needsAttention"
	| "degraded"
	| "unknown"
	| "notEvaluated";

export type HealthAssessmentCompleteness = "complete" | "partial";

export type HealthAssessmentSource =
	| "kubernetes"
	| "argoHealth"
	| "argoSync"
	| "warningEvent"
	| "containerRestart"
	| "provider"
	| "healthContract";

export interface HealthAssessmentEvidence {
	source: HealthAssessmentSource;
	raw: unknown;
	state?: HealthAssessmentState;
	current: boolean;
	reason: string;
}

export interface HealthAssessment {
	state: HealthAssessmentState;
	completeness: HealthAssessmentCompleteness;
	winningSources: HealthAssessmentSource[];
	reasons: string[];
	evidence: HealthAssessmentEvidence[];
}
