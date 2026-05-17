import type {
	DiscoveredResourceKind,
	ResourceEventSummary,
	ResourceSummary,
} from "../../lib/types";
import type { ChipVariant } from "./constants";

export interface ConditionRow {
	type: string;
	status: string;
	reason?: string;
	message?: string;
}

export function shouldFetchResourceDetails(
	resource: Pick<ResourceSummary, "cluster" | "kind" | "name">,
): boolean {
	return (
		Boolean(resource.cluster) &&
		Boolean(resource.kind) &&
		Boolean(resource.name)
	);
}

export function shouldFetchResourceEvents(
	resource: Pick<ResourceSummary, "cluster" | "kind" | "name">,
): boolean {
	return shouldFetchResourceDetails(resource);
}

export function dynamicResourceKindFromSummary(
	resource: ResourceSummary,
): DiscoveredResourceKind | null {
	if (
		!resource.dynamic ||
		!resource.apiVersion ||
		resource.version === undefined ||
		!resource.kind ||
		!resource.plural ||
		resource.namespaced === undefined
	) {
		return null;
	}

	return {
		group: resource.group ?? "",
		version: resource.version,
		apiVersion: resource.apiVersion,
		kind: resource.kind,
		plural: resource.plural,
		namespaced: resource.namespaced,
	};
}

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === "object" && value !== null && !Array.isArray(value);
}

export function getConditionRows(
	status: Record<string, unknown> | undefined,
): ConditionRow[] {
	if (!status || !Array.isArray(status.conditions)) return [];
	return status.conditions.filter(isRecord).map((condition) => ({
		type: String(condition.type ?? "Condition"),
		status: String(condition.status ?? "Unknown"),
		reason:
			typeof condition.reason === "string" ? condition.reason : undefined,
		message:
			typeof condition.message === "string" ? condition.message : undefined,
	}));
}

export interface IncidentSignal {
	id: string;
	label: string;
	value: string;
	tone: ChipVariant;
	source: "status" | "condition" | "event";
}

export function incidentSignalCardClassName(tone: ChipVariant): string {
	const base = "rounded-md border border-l-4 p-3";
	switch (tone) {
		case "error":
			return `${base} border-red-500/25 border-l-red-500 bg-red-500/5`;
		case "warning":
			return `${base} border-amber-500/25 border-l-amber-500 bg-amber-500/5`;
		case "info":
			return `${base} border-sky-500/25 border-l-sky-500 bg-sky-500/5`;
		case "success":
			return `${base} border-emerald-500/25 border-l-emerald-500 bg-emerald-500/5`;
		case "neutral":
			return `${base} border-border border-l-muted bg-card`;
	}
}

function eventWarningSummary(events: ResourceEventSummary[]): string | null {
	const warnings = events.filter((event) => event.eventType === "Warning");
	if (warnings.length === 0) return null;
	const reasons = new Set(warnings.map((event) => event.reason));
	const repeats = warnings.reduce((total, event) => total + event.count, 0);
	const reasonLabel = reasons.size === 1 ? "warning reason" : "warning reasons";
	const repeatLabel = repeats === 1 ? "repeat" : "repeats";
	return `${reasons.size} ${reasonLabel} · ${repeats} ${repeatLabel}`;
}

export function buildIncidentSignals(
	resource: ResourceSummary,
	conditions: ConditionRow[],
	events: ResourceEventSummary[],
): IncidentSignal[] {
	const signals: IncidentSignal[] = [];
	const status = resource.status?.toLowerCase();
	const ready = resource.ready?.toLowerCase();
	const restarts = resource.restarts ?? 0;

	if (
		resource.status &&
		["failed", "error", "pending", "terminating", "unknown"].includes(
			status ?? "",
		)
	) {
		signals.push({
			id: "status",
			label: "Status",
			value: resource.status,
			tone: status === "failed" || status === "error" ? "error" : "warning",
			source: "status",
		});
	}

	if (resource.ready && ready === "false") {
		signals.push({
			id: "ready",
			label: "Ready",
			value: resource.ready,
			tone: "error",
			source: "status",
		});
	}

	if (restarts > 0) {
		signals.push({
			id: "restarts",
			label: "Restarts",
			value: String(restarts),
			tone: restarts > 5 ? "error" : "warning",
			source: "status",
		});
	}

	for (const condition of conditions) {
		if (condition.status === "True") continue;
		signals.push({
			id: `condition:${condition.type}`,
			label: "Condition",
			value: `${condition.type}=${condition.status}${
				condition.reason ? ` · ${condition.reason}` : ""
			}`,
			tone: condition.status === "False" ? "error" : "warning",
			source: "condition",
		});
	}

	const warningSummary = eventWarningSummary(events);
	if (warningSummary) {
		signals.push({
			id: "events:warnings",
			label: "Warning events",
			value: warningSummary,
			tone: "warning",
			source: "event",
		});
	}

	return signals;
}

export const formatMetadata = (
	metadata: Record<string, unknown>,
): Array<{ key: string; value: unknown }> => {
	const entries: Array<{ key: string; value: unknown }> = [];
	if (metadata.name) entries.push({ key: "Name", value: metadata.name });
	if (metadata.namespace)
		entries.push({ key: "Namespace", value: metadata.namespace });
	if (metadata.uid) entries.push({ key: "UID", value: metadata.uid });
	if (metadata.resourceVersion)
		entries.push({
			key: "Resource Version",
			value: metadata.resourceVersion,
		});
	if (metadata.creationTimestamp)
		entries.push({ key: "Created", value: metadata.creationTimestamp });
	if (metadata.labels)
		entries.push({
			key: "Labels",
			value: metadata.labels,
		});
	if (metadata.annotations)
		entries.push({
			key: "Annotations",
			value: metadata.annotations,
		});
	return entries;
};

export const getErrorMessage = (err: unknown): string => {
	if (err instanceof Error) return err.message;
	if (typeof err === "string") return err;
	return "Unknown error";
};
