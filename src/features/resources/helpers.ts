import type { SortingState } from "@tanstack/react-table";
import type { ClusterScopedKind, ResourceSummary } from "@/lib/types";
import { CLUSTER_SCOPED_KINDS } from "@/lib/types";

export interface FetchKey {
	kind: string;
	namespace: string | undefined;
}

export interface HealthSummary {
	total: number;
	healthy: number;
	attention: number;
	degraded: number;
	restarted: number;
}

export interface ScopePill {
	label: string;
	value: string;
}

export function isClusterScopedKind(kind: string): kind is ClusterScopedKind {
	return (CLUSTER_SCOPED_KINDS as readonly string[]).includes(kind);
}

export function buildFetchKeys(
	namespaces: string[],
	kinds: string[],
): FetchKey[] {
	const keys: FetchKey[] = [];
	for (const kind of kinds) {
		if (isClusterScopedKind(kind)) {
			keys.push({ kind, namespace: undefined });
			continue;
		}
		for (const namespace of namespaces) {
			keys.push({ kind, namespace });
		}
	}
	return keys;
}

export function sortedRows(
	data: ResourceSummary[],
	sorting: SortingState,
): ResourceSummary[] {
	if (sorting.length === 0) return data;
	return [...data].sort((a, b) => {
		for (const { id, desc } of sorting) {
			const av = (a as unknown as Record<string, unknown>)[id];
			const bv = (b as unknown as Record<string, unknown>)[id];
			if (av == null && bv == null) continue;
			if (av == null) return desc ? 1 : -1;
			if (bv == null) return desc ? -1 : 1;
			const cmp = String(av).localeCompare(String(bv));
			if (cmp !== 0) return desc ? -cmp : cmp;
		}
		return 0;
	});
}

export function filterResources(
	data: ResourceSummary[],
	search: string,
	argoAppFilter: string,
): ResourceSummary[] {
	const term = search.trim().toLowerCase();
	return data.filter((resource) => {
		if (argoAppFilter && resource.argoApp !== argoAppFilter) return false;
		if (!term) return true;
		return (
			resource.name.toLowerCase().includes(term) ||
			resource.namespace?.toLowerCase().includes(term) === true ||
			resource.kind.toLowerCase().includes(term) ||
			resource.ownerRef?.toLowerCase().includes(term) === true ||
			resource.argoApp?.toLowerCase().includes(term) === true ||
			resource.helmRelease?.toLowerCase().includes(term) === true
		);
	});
}

export function uniqueArgoApps(data: ResourceSummary[]): string[] {
	return Array.from(
		new Set(
			data.map((resource) => resource.argoApp).filter((app): app is string =>
				Boolean(app),
			),
		),
	).sort((a, b) => a.localeCompare(b));
}

export function formatResourceGroupLabel(resource: ResourceSummary): string {
	return resource.argoApp
		? `Managed by Argo app: ${resource.argoApp}`
		: "Unmanaged resources";
}

export function formatResourceTypeGroupLabel(resource: ResourceSummary): string {
	if (resource.kind.endsWith("s")) return `${resource.kind}es`;
	if (resource.kind.endsWith("y")) return `${resource.kind.slice(0, -1)}ies`;
	return `${resource.kind}s`;
}

export function resourceGroupCollapseKey(resource: ResourceSummary): string {
	return `app:${formatResourceGroupLabel(resource)}`;
}

export function resourceTypeGroupCollapseKey(
	resource: ResourceSummary,
): string {
	return `${resourceGroupCollapseKey(resource)}::type:${formatResourceTypeGroupLabel(resource)}`;
}

export function describeResourceScope(
	clusterContext: string,
	namespaces: string[],
	kinds: string[],
	argoAppFilter: string,
): ScopePill[] {
	const pills: ScopePill[] = [{ label: "Context", value: clusterContext }];
	if (namespaces.length > 0) {
		pills.push({
			label: namespaces.length === 1 ? "Namespace" : "Namespaces",
			value:
				namespaces.length <= 2
					? namespaces.join(", ")
					: `${namespaces.slice(0, 2).join(", ")} +${namespaces.length - 2}`,
		});
	}
	if (kinds.length > 0) {
		pills.push({
			label: kinds.length === 1 ? "Kind" : "Kinds",
			value:
				kinds.length <= 3
					? kinds.join(", ")
					: `${kinds.slice(0, 3).join(", ")} +${kinds.length - 3}`,
		});
	}
	if (argoAppFilter) {
		pills.push({ label: "Argo app", value: argoAppFilter });
	}
	return pills;
}

export function buildResourceHealthSummary(
	rows: ResourceSummary[],
): HealthSummary {
	return rows.reduce<HealthSummary>(
		(summary, row) => {
			const status = row.status?.toLowerCase() ?? "";
			const ready = row.ready?.toLowerCase() ?? "";
			const restarts = row.restarts ?? 0;
			const isDegraded =
				status === "failed" ||
				status === "error" ||
				status === "crashloopbackoff" ||
				status === "imagepullbackoff" ||
				ready === "false";
			const needsAttention =
				!isDegraded &&
				(status === "pending" ||
					status === "terminating" ||
					status === "unknown" ||
					restarts > 0);
			const isHealthy =
				!isDegraded &&
				!needsAttention &&
				(status === "running" ||
					status === "succeeded" ||
					status === "ready" ||
					ready === "true");

			return {
				total: summary.total + 1,
				healthy: summary.healthy + (isHealthy ? 1 : 0),
				attention: summary.attention + (needsAttention ? 1 : 0),
				degraded: summary.degraded + (isDegraded ? 1 : 0),
				restarted: summary.restarted + (restarts > 0 ? 1 : 0),
			};
		},
		{ total: 0, healthy: 0, attention: 0, degraded: 0, restarted: 0 },
	);
}

export function tableTooltipText(
	value: string | number | null | undefined,
): string {
	return value === undefined || value === null || value === ""
		? "—"
		: String(value);
}
