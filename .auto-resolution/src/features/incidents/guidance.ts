import {
	buildIncidentSignals,
	getConditionRows,
	getContainerStatusRows,
} from "@/features/resource-detail";
import type { PathStateDetailTab } from "@/lib/path-state";
import type {
	IncidentCockpitItem,
	ResourceDetailsFull,
	ResourceSummary,
} from "@/lib/types";
import {
	buildIncidentAvailableActions,
	type IncidentAvailableAction,
	type IncidentEnrichmentState,
	type IncidentOwnerResolution,
} from "./incident-actions";
import {
	incidentCaseSummary,
	incidentCaseTitle,
} from "./model";

export {
	buildIncidentAvailableActions,
	resolveIncidentOwner,
	type IncidentAvailableAction,
	type IncidentEnrichmentState,
	type IncidentOwnerResolution,
} from "./incident-actions";
export type IncidentEvidenceTone = "error" | "warning" | "info" | "neutral";

export interface IncidentEvidence {
	id: string;
	label: string;
	detail: string;
	timestamp?: string;
	tone: IncidentEvidenceTone;
}

export interface IncidentGuideFact {
	label: "Impact" | "Confidence" | "Scope";
	value: string;
	detail: string;
}

export interface IncidentInvestigationStep {
	id: string;
	title: string;
	description: string;
	target: ResourceSummary;
	tab: PathStateDetailTab;
}

export interface IncidentGuidance {
	title: string;
	summary: string;
	owner: ResourceSummary | null;
	facts: IncidentGuideFact[];
	evidence: IncidentEvidence[];
	missing: string[];
	steps: IncidentInvestigationStep[];
	actions: IncidentAvailableAction[];
}

function resourceLabel(resource: ResourceSummary): string {
	return `${resource.kind}/${resource.name}`;
}

function signalTone(tone: string): IncidentEvidenceTone {
	if (tone === "error") return "error";
	if (tone === "warning") return "warning";
	if (tone === "info") return "info";
	return "neutral";
}

function snapshotEvidence(item: IncidentCockpitItem): IncidentEvidence[] {
	const evidence = item.signals.map((signal, index) => ({
		id: `snapshot:${index}:${signal.kind}`,
		label: signal.label,
		detail: signal.message || signal.source,
		timestamp: signal.lastSeenAt,
		tone: item.severity === "degraded" ? "error" as const : "warning" as const,
	}));
	if (
		item.latestWarningEvent &&
		!evidence.some((entry) =>
			entry.label.trim().toLowerCase() === item.latestWarningEvent?.reason.trim().toLowerCase() &&
			entry.detail.trim().toLowerCase() === item.latestWarningEvent?.message.trim().toLowerCase()
		)
	) {
		evidence.push({
			id: "snapshot:warning-event",
			label: item.latestWarningEvent.reason,
			detail: item.latestWarningEvent.message,
			timestamp: item.latestWarningEvent.lastSeenAt,
			tone: "warning",
		});
	}
	return evidence;
}

function detailedEvidence(
	details: ResourceDetailsFull | undefined,
): IncidentEvidence[] {
	if (!details) return [];
	const conditions = getConditionRows(details.status);
	const containers = getContainerStatusRows(details.status);
	return buildIncidentSignals(details.summary, conditions, [], containers).map((signal) => ({
		id: `detail:${signal.id}`,
		label: signal.label,
		detail: signal.value,
		timestamp: signal.valueParts?.find((part) => part.kind === "timestamp")?.value,
		tone: signalTone(signal.tone),
	}));
}

function mergeEvidence(
	detail: IncidentEvidence[],
	snapshot: IncidentEvidence[],
): IncidentEvidence[] {
	const seen = new Set<string>();
	return [...detail, ...snapshot].filter((entry) => {
		const key = `${entry.label.trim().toLowerCase()}\u0000${entry.detail.trim().toLowerCase()}\u0000${entry.timestamp ?? ""}`;
		if (seen.has(key)) return false;
		seen.add(key);
		return true;
	}).slice(0, 6);
}

function investigationSteps(
	item: IncidentCockpitItem,
	owner: ResourceSummary | null,
): IncidentInvestigationStep[] {
	const steps: IncidentInvestigationStep[] = [];
	if (item.resource.kind === "Pod" && item.resource.namespace) {
		steps.push({
			id: "logs",
			title: "Read container logs",
			description: "Look for the application error before changing the Pod or its owner.",
			target: item.resource,
			tab: "logs",
		});
	}
	steps.push(
		{
			id: "events",
			title: "Review events",
			description: "Check scheduling, kubelet, admission, and repeated warning evidence.",
			target: item.resource,
			tab: "events",
		},
		{
			id: "details",
			title: "Inspect resource details",
			description: "Review conditions, container state, metadata, and lifecycle fields.",
			target: item.resource,
			tab: "details",
		},
	);
	if (owner) {
		steps.push({
			id: "owner-details",
			title: `Inspect owning ${owner.kind}`,
			description: "Compare owner health and sibling replicas before changing workload state.",
			target: owner,
			tab: "details",
		});
	}
	steps.push({
		id: "yaml",
		title: "Inspect YAML",
		description: "Confirm desired configuration and controller references.",
		target: item.resource,
		tab: "yaml",
	});
	return steps;
}

export function buildIncidentGuidance(
	item: IncidentCockpitItem,
	details: ResourceDetailsFull | undefined,
	ownerResolution: IncidentOwnerResolution,
	detailsState: IncidentEnrichmentState,
	topologyState: IncidentEnrichmentState,
): IncidentGuidance {
	const ownershipConfirmed = topologyState === "ready" && ownerResolution.complete;
	const owner = ownershipConfirmed
		? ownerResolution.workloadOwner ?? ownerResolution.directOwner
		: null;
	const evidence = mergeEvidence(detailedEvidence(details), snapshotEvidence(item));
	const firstEvidence = evidence[0];
	const title = firstEvidence
		? `${firstEvidence.label}: ${firstEvidence.detail}`
		: incidentCaseTitle(item);
	const ownerSummary = item.resource.kind !== "Pod"
		? ""
		: owner
			? ` Controller owner resolves to ${resourceLabel(owner)}.`
			: " Controller ownership is not confirmed.";
	const missing = ["Root cause is not confirmed by current evidence."];
	if (detailsState === "loading") missing.push("Live conditions and container state are still loading.");
	if (detailsState === "error") missing.push("Live conditions and container state could not be loaded.");
	if (item.resource.kind === "Pod" && topologyState === "loading") {
		missing.push("Controller ownership is still loading.");
	}
	if (item.resource.kind === "Pod" && topologyState === "error") {
		missing.push("Controller ownership could not be loaded; owner actions are unavailable.");
	}
	if (!item.latestWarningEvent) missing.push("No Warning event is present in the incident snapshot.");

	return {
		title,
		summary: `${incidentCaseSummary(item)}${ownerSummary}`,
		owner,
		facts: [
			{
				label: "Impact",
				value: resourceLabel(item.resource),
				detail: "Current signal is scoped to this exact resource.",
			},
			{
				label: "Confidence",
				value: details ? "Live detail evidence" : "Incident snapshot",
				detail: details ? "Conditions and container state were loaded." : "No live detail claim is inferred.",
			},
			{
				label: "Scope",
				value: item.resource.namespace ?? "Cluster scoped",
				detail: owner ? `${item.resource.cluster} · owner ${resourceLabel(owner)}` : item.resource.cluster,
			},
		],
		evidence,
		missing,
		steps: investigationSteps(item, owner),
		actions: buildIncidentAvailableActions(item, ownerResolution, topologyState),
	};
}
