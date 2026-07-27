import type { GitOpsData } from "./surfaceModel";

export interface GitOpsSummaryFact {
	label: string;
	value: number;
	tone?: "healthy" | "unhealthy";
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
		const ready = data.flux.filter((item) => item.readyStatus === "True").length;
		const notReady = data.flux.filter((item) => item.readyStatus === "False").length;
		return {
			activeProvider: "Flux",
			detectedProviders,
			totalObjects,
			facts: [
				{ label: "Resources", value: data.flux.length },
				{ label: "Kinds", value: new Set(data.flux.map((item) => item.resourceKind.kind)).size },
				{ label: "Ready", value: ready, tone: "healthy" },
				{ label: "Not Ready", value: notReady, tone: "unhealthy" },
				{ label: "Unknown", value: data.flux.length - ready - notReady },
			],
		};
	}

	return {
		activeProvider: "Argo CD",
		detectedProviders,
		totalObjects,
		facts: [
			{ label: "Applications", value: data.apps.length },
			{ label: "ApplicationSets", value: data.appSets.length },
			{ label: "AppProjects", value: data.projects.length },
			{
				label: "Synced",
				value: data.apps.filter((item) => item.syncStatus === "Synced").length,
				tone: "healthy",
			},
			{
				label: "Degraded",
				value: data.apps.filter((item) => item.healthStatus === "Degraded").length,
				tone: "unhealthy",
			},
		],
	};
}
