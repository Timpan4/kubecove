import type {
	ArgoApplicationHistory,
	ArgoApplicationRef,
	ArgoManagedResource,
	ArgoOperationRequest,
	ArgoResourceComparison,
} from "@/lib/gitops-types";
import type { JsonValue } from "@/lib/types";

export interface ArgoResourceCounts {
	total: number;
	needsSync: number;
	current: number;
	healthy: number;
	degraded: number;
	progressing: number;
	prune: number;
}

export type ArgoResourceFilter =
	| "none"
	| "allManaged"
	| "needsSync"
	| "healthy"
	| "degraded"
	| "progressing"
	| "prune";

type ArgoResourceIdentity = Pick<ArgoManagedResource, "group" | "kind" | "namespace" | "name"> & {
	apiVersion?: string | null;
};

export function argoResourceCounts(resources: ArgoManagedResource[]): ArgoResourceCounts {
	const total = resources.length;
	const current = resources.filter(
		(resource) => normalized(resource.status) === "synced",
	).length;
	return {
		total,
		needsSync: total - current,
		current,
		healthy: resources.filter((resource) => normalized(resource.health) === "healthy").length,
		degraded: resources.filter((resource) => normalized(resource.health) === "degraded").length,
		progressing: resources.filter((resource) => normalized(resource.health) === "progressing").length,
		prune: resources.filter((resource) => resource.requiresPruning === true).length,
	};
}

export function argoResourceIdentityKey(resource: ArgoResourceIdentity): string | null {
	const kind = normalized(resource.kind);
	const name = normalized(resource.name);
	if (!kind || !name) return null;
	return `${normalizedGroup(resource)}:${kind}:${normalized(resource.namespace)}:${name}`;
}

export function argoResourceMatchesFilter(
	resource: ArgoManagedResource,
	filter: ArgoResourceFilter,
): boolean {
	if (filter === "none" || filter === "allManaged") return true;
	if (filter === "needsSync") return normalized(resource.status) !== "synced";
	if (filter === "prune") return resource.requiresPruning === true;
	return normalized(resource.health) === filter;
}

export function filterWorkspaceResourcesByArgo<T extends ArgoResourceIdentity>(
	resources: T[],
	managedResources: ArgoManagedResource[],
	filter: ArgoResourceFilter,
): T[] {
	if (filter === "none") return resources;
	const managedKeys = new Set(
		managedResources
			.filter((resource) => argoResourceMatchesFilter(resource, filter))
			.map(argoResourceIdentityKey)
			.filter((key): key is string => key !== null),
	);
	return resources.filter((resource) => {
		const key = argoResourceIdentityKey(resource);
		return key !== null && managedKeys.has(key);
	});
}

export function argoReconciliationResources(resources: ArgoManagedResource[]): ArgoManagedResource[] {
	return resources.filter(
		(resource) =>
			normalized(resource.status) !== "synced" ||
			normalized(resource.health) === "degraded" ||
			normalized(resource.health) === "progressing" ||
			resource.requiresPruning === true,
	);
}

export function preserveArgoResourceSelection<T extends ArgoResourceIdentity>(
	selected: T | null,
	resources: T[],
): T | null {
	if (!selected) return null;
	const key = argoResourceIdentityKey(selected);
	return key === null ? null : resources.find((resource) => argoResourceIdentityKey(resource) === key) ?? null;
}

export function argoHistoryKey(application: ArgoApplicationRef, entry: ArgoApplicationHistory): string {
	const applicationKey = `${normalized(application.namespace)}:${normalized(application.name)}`;
	const operationId = entry.id;
	if (operationId !== null && operationId !== undefined && Number.isFinite(operationId)) {
		return `${applicationKey}:id:${operationId}`;
	}
	const revision = normalized(entry.revision) || entry.revisions?.map(normalized).filter(Boolean).join(",") || "unknown";
	return `${applicationKey}:revision:${revision}`;
}

export function preserveArgoHistorySelection(
	application: ArgoApplicationRef,
	history: ArgoApplicationHistory[],
	selected: string | null,
): string | null {
	if (!history.length) return null;
	const keys = history.map((entry) => argoHistoryKey(application, entry));
	return selected && keys.includes(selected) ? selected : keys[0];
}

export interface ArgoComparisonDocument {
	target: JsonValue | undefined;
	desired: JsonValue | undefined;
	live: JsonValue | undefined;
	normalizedLive: JsonValue | undefined;
	modified: boolean | null | undefined;
	exact: boolean | null | undefined;
	provenance: string | null | undefined;
}

export function argoComparisonForResource(
	comparisons: ArgoResourceComparison[],
	resource: ArgoManagedResource,
): ArgoResourceComparison | null {
	const key = argoResourceIdentityKey(resource);
	return key === null
		? null
		: comparisons.find((comparison) => argoResourceIdentityKey(comparison.resource) === key) ?? null;
}

export function argoComparisonDocument(
	resource: ArgoManagedResource,
	comparison?: ArgoResourceComparison | null,
): ArgoComparisonDocument {
	const target = comparison?.targetState ?? resource.targetState;
	const live = comparison?.liveState ?? resource.liveState;
	return {
		target,
		desired: target,
		live,
		normalizedLive: comparison?.normalizedLiveState ?? live,
		modified: comparison?.modified,
		exact: comparison?.exact,
		provenance: comparison?.provenance,
	};
}

export interface ArgoSyncSettings {
	revision: string;
	prune: boolean;
	dryRun: boolean;
	force: boolean;
}

export const defaultArgoSyncSettings: ArgoSyncSettings = {
	revision: "",
	prune: false,
	dryRun: false,
	force: false,
};

export function argoSyncNeedsConfirmation(
	settings: ArgoSyncSettings,
	defaults: ArgoSyncSettings = defaultArgoSyncSettings,
): boolean {
	return (
		settings.revision.trim() !== defaults.revision.trim() ||
		settings.prune !== defaults.prune ||
		settings.dryRun !== defaults.dryRun ||
		settings.force !== defaults.force
	);
}

export function applyArgoSyncDefaults(
	settings: ArgoSyncSettings,
	previousDefaults: ArgoSyncSettings,
	nextDefaults: ArgoSyncSettings,
): ArgoSyncSettings {
	return argoSyncNeedsConfirmation(settings, previousDefaults)
		? settings
		: { ...nextDefaults };
}

export function withArgoSyncSettings(
	request: ArgoOperationRequest,
	settings: ArgoSyncSettings,
): ArgoOperationRequest {
	return {
		...request,
		action: "sync",
		revision: settings.revision.trim() || null,
		resources: [],
		prune: settings.prune,
		dryRun: settings.dryRun,
		force: settings.force,
	};
}

function normalizedGroup(resource: ArgoResourceIdentity): string {
	if (resource.group !== undefined && resource.group !== null) return normalized(resource.group);
	const apiVersion = resource.apiVersion;
	return normalized(apiVersion?.includes("/") ? apiVersion.split("/", 1)[0] : "");
}

function normalized(value: string | null | undefined): string {
	return value?.trim().toLowerCase() ?? "";
}
