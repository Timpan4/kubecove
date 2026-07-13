import type { ResourceListRequest, ResourceSummary } from "./types";

export const RECENT_ENTRY_LIMIT = 8;

export type WorkspaceEntryPointKind = "namespace" | "app" | "resource";

export interface WorkspaceEntryPoint {
	kind: WorkspaceEntryPointKind;
	clusterContext: string;
	namespace?: string;
	name: string;
	resourceKind?: string;
	apiVersion?: string;
	lastVisitedAt: string;
}

export interface WorkspaceEntryPoints {
	pinned: WorkspaceEntryPoint[];
	recent: WorkspaceEntryPoint[];
}

export interface ResourceEntryPointCoverage {
	clusterContext: string;
	requests: ResourceListRequest[];
}

export function entryPointFromNamespace(
	clusterContext: string,
	namespace: string,
	now = new Date().toISOString(),
): WorkspaceEntryPoint {
	return {
		kind: "namespace",
		clusterContext,
		namespace,
		name: namespace,
		lastVisitedAt: now,
	};
}

export function entryPointFromApplication(
	clusterContext: string,
	name: string,
	namespace?: string | null,
	now = new Date().toISOString(),
): WorkspaceEntryPoint {
	return {
		kind: "app",
		clusterContext,
		namespace: namespace ?? undefined,
		name,
		resourceKind: "Application",
		apiVersion: "argoproj.io/v1alpha1",
		lastVisitedAt: now,
	};
}

export function entryPointFromResource(
	resource: ResourceSummary,
	now = new Date().toISOString(),
): WorkspaceEntryPoint {
	return {
		kind: "resource",
		clusterContext: resource.cluster,
		namespace: resource.namespace ?? undefined,
		name: resource.name,
		resourceKind: resource.kind,
		apiVersion: resource.apiVersion,
		lastVisitedAt: now,
	};
}

export function entryPointKey(entry: WorkspaceEntryPoint): string {
	return [
		entry.kind,
		entry.clusterContext,
		entry.namespace ?? "",
		entry.resourceKind ?? "",
		entry.apiVersion ?? "",
		entry.name,
	].join("|");
}

export function normalizeEntryPoints(
	entries?: Partial<WorkspaceEntryPoints>,
): WorkspaceEntryPoints {
	return {
		pinned: Array.isArray(entries?.pinned) ? deduplicate(entries.pinned) : [],
		recent: Array.isArray(entries?.recent)
			? deduplicate(entries.recent).slice(0, RECENT_ENTRY_LIMIT)
			: [],
	};
}

export function isPinnedEntry(
	entries: Partial<WorkspaceEntryPoints> | undefined,
	entry: WorkspaceEntryPoint,
): boolean {
	const key = entryPointKey(entry);
	return normalizeEntryPoints(entries).pinned.some((item) => entryPointKey(item) === key);
}

export function togglePinnedEntry(
	entries: Partial<WorkspaceEntryPoints> | undefined,
	entry: WorkspaceEntryPoint,
): WorkspaceEntryPoints {
	const normalized = normalizeEntryPoints(entries);
	const key = entryPointKey(entry);
	const pinned = normalized.pinned.some((item) => entryPointKey(item) === key)
		? normalized.pinned.filter((item) => entryPointKey(item) !== key)
		: [entry, ...normalized.pinned];
	return { ...normalized, pinned };
}

export function recordRecentEntry(
	entries: Partial<WorkspaceEntryPoints> | undefined,
	entry: WorkspaceEntryPoint,
): WorkspaceEntryPoints {
	const normalized = normalizeEntryPoints(entries);
	const key = entryPointKey(entry);
	return {
		...normalized,
		recent: [entry, ...normalized.recent.filter((item) => entryPointKey(item) !== key)].slice(
			0,
			RECENT_ENTRY_LIMIT,
		),
	};
}

export function entryPointsEqual(
	left: Partial<WorkspaceEntryPoints> | undefined,
	right: Partial<WorkspaceEntryPoints> | undefined,
): boolean {
	return JSON.stringify(normalizeEntryPoints(left)) === JSON.stringify(normalizeEntryPoints(right));
}

export function reconcileEntryPoints(
	entries: Partial<WorkspaceEntryPoints> | undefined,
	liveResources: ResourceSummary[],
	coverage: ResourceEntryPointCoverage[],
): WorkspaceEntryPoints {
	const normalized = normalizeEntryPoints(entries);
	const exists = new Set(
		liveResources.map((resource) => entryPointKey(entryPointFromResource(resource, ""))),
	);
	const covered = (entry: WorkspaceEntryPoint) =>
		coverage.some(
			(plan) =>
				plan.clusterContext === entry.clusterContext &&
				plan.requests.some((request) => resourceRequestCoversEntry(request, entry)),
		);
	const retained = (entry: WorkspaceEntryPoint) =>
		entry.kind !== "resource" || !covered(entry) || exists.has(entryPointKey(entry));
	return {
		pinned: normalized.pinned.filter(retained),
		recent: normalized.recent.filter(retained),
	};
}

export function resourceFromEntryPoint(entry: WorkspaceEntryPoint): ResourceSummary | null {
	if (entry.kind !== "resource" || !entry.resourceKind) return null;
	return {
		cluster: entry.clusterContext,
		kind: entry.resourceKind,
		name: entry.name,
		namespace: entry.namespace ?? null,
		age: "",
		health: "unknown",
		apiVersion: entry.apiVersion,
	};
}

function resourceRequestCoversEntry(
	request: ResourceListRequest,
	entry: WorkspaceEntryPoint,
): boolean {
	if (entry.kind !== "resource" || !entry.resourceKind) return false;
	if (request.namespace !== undefined && request.namespace !== entry.namespace) return false;
	if (request.kind) return request.kind === entry.resourceKind;
	const resourceKind = request.resourceKind;
	if (!resourceKind || resourceKind.kind !== entry.resourceKind) return false;
	return !entry.apiVersion || resourceKind.apiVersion === entry.apiVersion;
}

function deduplicate(entries: WorkspaceEntryPoint[]): WorkspaceEntryPoint[] {
	const seen = new Set<string>();
	return entries.filter((entry) => {
		const key = entryPointKey(entry);
		if (seen.has(key)) return false;
		seen.add(key);
		return true;
	});
}
