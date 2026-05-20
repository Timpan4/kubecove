import type { SortingState } from "@tanstack/react-table";
import type {
	ClusterScopedKind,
	DiscoveredResourceKind,
	ResourceKindSelection,
	ResourceSummary,
	WatchResourceKey,
} from "@/lib/types";
import { classifyResourceHealth } from "@/lib/resource-health";
import { CLUSTER_SCOPED_KINDS } from "@/lib/types";

export interface FetchKey {
	kind: ResourceKindSelection;
	namespace: string | undefined;
}

export interface HealthSummary {
	total: number;
	healthy: number;
	attention: number;
	degraded: number;
	restarted: number;
}

export type HealthFilter =
	| "all"
	| "unhealthy"
	| "attention"
	| "degraded"
	| "restarted";

export interface ScopePill {
	label: string;
	value: string;
}

const TOPOLOGY_WATCH_KINDS = [
	"Deployment",
	"DaemonSet",
	"ReplicaSet",
	"StatefulSet",
	"CronJob",
	"Job",
	"Pod",
	"PersistentVolumeClaim",
	"Service",
	"Ingress",
	"ConfigMap",
	"Secret",
] as const;

export function resourceSelectionKey(resource: ResourceSummary): string {
	return `${resource.cluster}:${resource.apiVersion ?? ""}:${resource.kind}:${resource.namespace ?? ""}:${resource.name}`;
}

export function isClusterScopedKind(kind: string): kind is ClusterScopedKind {
	return (CLUSTER_SCOPED_KINDS as readonly string[]).includes(kind);
}

export function isDiscoveredResourceKind(
	kind: ResourceKindSelection,
): kind is DiscoveredResourceKind {
	return typeof kind !== "string";
}

export function resourceKindLabel(kind: ResourceKindSelection): string {
	return isDiscoveredResourceKind(kind) ? kind.kind : kind;
}

export function resourceKindFetchKey(kind: ResourceKindSelection): string {
	return isDiscoveredResourceKind(kind)
		? `dynamic:${kind.apiVersion}:${kind.plural}:${kind.kind}`
		: `typed:${kind}`;
}

function isClusterScopedSelection(kind: ResourceKindSelection): boolean {
	return isDiscoveredResourceKind(kind)
		? !kind.namespaced
		: isClusterScopedKind(kind);
}

export function buildFetchKeys(
	namespaces: string[],
	kinds: ResourceKindSelection[],
): FetchKey[] {
	const keys: FetchKey[] = [];
	for (const kind of kinds) {
		if (isClusterScopedSelection(kind)) {
			keys.push({ kind, namespace: undefined });
			continue;
		}
		if (isDiscoveredResourceKind(kind) && namespaces.length === 0) {
			keys.push({ kind, namespace: undefined });
			continue;
		}
		for (const namespace of namespaces) {
			keys.push({ kind, namespace });
		}
	}
	return keys;
}

export function watchKeysFromFetchKeys(keys: FetchKey[]): WatchResourceKey[] {
	return keys.map((key) => {
		if (isDiscoveredResourceKind(key.kind)) {
			return {
				resourceKind: {
					kind: key.kind.kind,
					group: key.kind.group,
					version: key.kind.version,
					apiVersion: key.kind.apiVersion,
					plural: key.kind.plural,
					namespaced: key.kind.namespaced,
				},
				namespace: key.namespace,
			};
		}

		return {
			resourceKind: { kind: key.kind },
			namespace: key.namespace,
		};
	});
}

function watchKeySignature(key: WatchResourceKey): string {
	const kind = key.resourceKind;
	return [
		kind.kind,
		kind.apiVersion ?? "",
		kind.plural ?? "",
		key.namespace ?? "",
	].join(":");
}

export function mergeWatchKeys(
	...groups: WatchResourceKey[][]
): WatchResourceKey[] {
	const merged = new Map<string, WatchResourceKey>();
	for (const group of groups) {
		for (const key of group) {
			merged.set(watchKeySignature(key), key);
		}
	}
	return Array.from(merged.values());
}

export function topologyWatchKeys(namespaces: string[]): WatchResourceKey[] {
	const namespaceScopes: Array<string | undefined> =
		namespaces.length > 0 ? namespaces : [undefined];
	return TOPOLOGY_WATCH_KINDS.flatMap((kind) =>
		namespaceScopes.map((namespace) => ({
			resourceKind: { kind },
			namespace,
		})),
	);
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
			const cmp =
				typeof av === "number" && typeof bv === "number"
					? av - bv
					: String(av).localeCompare(String(bv), undefined, {
							numeric: true,
							sensitivity: "base",
						});
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
			resource.apiVersion?.toLowerCase().includes(term) === true ||
			resource.group?.toLowerCase().includes(term) === true ||
			resource.plural?.toLowerCase().includes(term) === true ||
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
	if (resource.argoApp) return `Managed by Argo app: ${resource.argoApp}`;
	if (resource.ownerRef) return `Owned by: ${resource.ownerRef}`;
	return "Unmanaged resources";
}

export function formatResourceTypeGroupLabel(resource: ResourceSummary): string {
	if (resource.kind.endsWith("s")) return `${resource.kind}es`;
	if (resource.kind.endsWith("y")) return `${resource.kind.slice(0, -1)}ies`;
	return `${resource.kind}s`;
}

export function resourceGroupCollapseKey(resource: ResourceSummary): string {
	return `group:${formatResourceGroupLabel(resource)}`;
}

export function resourceTypeGroupCollapseKey(
	resource: ResourceSummary,
): string {
	return `${resourceGroupCollapseKey(resource)}::type:${formatResourceTypeGroupLabel(resource)}`;
}

export function describeResourceScope(
	clusterContext: string,
	namespaces: string[],
	kinds: ResourceKindSelection[],
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
					? kinds.map(resourceKindLabel).join(", ")
					: `${kinds.slice(0, 3).map(resourceKindLabel).join(", ")} +${
							kinds.length - 3
						}`,
		});
	}
	if (argoAppFilter) {
		pills.push({ label: "Argo app", value: argoAppFilter });
	}
	return pills;
}

export function filterResourcesByHealth(
	rows: ResourceSummary[],
	filter: HealthFilter,
): ResourceSummary[] {
	if (filter === "all") return rows;
	return rows.filter((row) => {
		const health = classifyResourceHealth(row);
		if (filter === "unhealthy") return health.degraded || health.attention;
		return health[filter];
	});
}

export function buildResourceHealthSummary(
	rows: ResourceSummary[],
): HealthSummary {
	return rows.reduce<HealthSummary>(
		(summary, row) => {
			const flags = classifyResourceHealth(row);

			return {
				total: summary.total + 1,
				healthy: summary.healthy + (flags.healthy ? 1 : 0),
				attention: summary.attention + (flags.attention ? 1 : 0),
				degraded: summary.degraded + (flags.degraded ? 1 : 0),
				restarted: summary.restarted + (flags.restarted ? 1 : 0),
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
