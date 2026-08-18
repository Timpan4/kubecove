import type { GitOpsData } from "./surfaceModel";

export interface GitOpsSummaryFact {
	label: string;
	value: number;
	tone?: "healthy" | "degraded";
}

export interface GitOpsSummary {
	activeProvider: "Argo CD" | "Flux";
	detectedProviders: ("Argo CD" | "Flux")[];
	totalObjects: number;
	facts: GitOpsSummaryFact[];
}

export function buildGitOpsSummary(data: GitOpsData, activeRailKey: string): GitOpsSummary {
	const detectedProviders: GitOpsSummary["detectedProviders"] = [];
	if (
		data.argoDetected === true ||
		(data.argoDetected !== false && data.apps.length + data.appSets.length + data.projects.length > 0)
	) {
		detectedProviders.push("Argo CD");
	}
	if (data.fluxDetected === true || (data.fluxDetected !== false && data.flux.length > 0)) {
		detectedProviders.push("Flux");
	}

	const totalObjects = data.apps.length + data.appSets.length + data.projects.length + data.flux.length;
	if (activeRailKey.startsWith("flux:")) {
		const healthy = data.flux.filter((item) => item.healthAssessment.state === "healthy").length;
		const degraded = data.flux.filter((item) => item.healthAssessment.state === "degraded").length;
		const attention = data.flux.filter(
			(item) => item.healthAssessment.state === "needsAttention",
		).length;
		const unknown = data.flux.filter((item) => item.healthAssessment.state === "unknown").length;
		const notEvaluated = data.flux.filter(
			(item) => item.healthAssessment.state === "notEvaluated",
		).length;
		return {
			activeProvider: "Flux",
			detectedProviders,
			totalObjects,
			facts: [
				{ label: "Resources", value: data.flux.length },
				{ label: "Kinds", value: new Set(data.flux.map((item) => item.resourceKind.kind)).size },
				{ label: "Healthy", value: healthy, tone: "healthy" },
				{ label: "Needs attention", value: attention },
				{ label: "Degraded", value: degraded, tone: "degraded" },
				{ label: "Unknown", value: unknown },
				{ label: "Not evaluated", value: notEvaluated },
			],
		};
	}
	const argoItems = [...data.apps, ...data.appSets, ...data.projects];

	return {
		activeProvider: "Argo CD",
		detectedProviders,
		totalObjects,
		facts: [
			{ label: "Applications", value: data.apps.length },
			{ label: "ApplicationSets", value: data.appSets.length },
			{ label: "AppProjects", value: data.projects.length },
			{ label: "Healthy", value: argoItems.filter((item) => item.healthAssessment?.state === "healthy").length, tone: "healthy" },
			{ label: "Needs attention", value: argoItems.filter((item) => item.healthAssessment?.state === "needsAttention").length },
			{ label: "Degraded", value: argoItems.filter((item) => item.healthAssessment?.state === "degraded").length, tone: "degraded" },
			{ label: "Unknown", value: argoItems.filter((item) => !item.healthAssessment || item.healthAssessment.state === "unknown").length },
			{ label: "Not evaluated", value: argoItems.filter((item) => item.healthAssessment?.state === "notEvaluated").length },
		],
	};
}
