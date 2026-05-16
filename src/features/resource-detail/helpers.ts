import type { DiscoveredResourceKind, ResourceSummary } from "../../lib/types";

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
