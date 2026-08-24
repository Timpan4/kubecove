import type { StatusTone } from "@/components/status-badge-styles";
import {
	gitOpsOwnership,
	gitOpsOwnershipFilterValue,
	gitOpsOwnershipGroupLabel,
} from "@/lib/gitops-ownership-evidence";
import { classifyResourceHealth } from "@/lib/resource-health";
import type {
	ArgoApplicationSummary,
	ClusterScopedKind,
	DiscoveredResourceKind,
	ResourceKindSelection,
	ResourceSummary,
	WatchResourceKey,
} from "@/lib/types";
import { CLUSTER_SCOPED_KINDS } from "@/lib/types";
import type { ResourceSortingState } from "./table-state";

export interface FetchKey {
	kind: ResourceKindSelection;
	namespace: string | undefined;
}

export interface HealthSummary {
	total: number;
	healthy: number;
	attention: number;
	degraded: number;
	unknown: number;
	notEvaluated: number;
	restartEvidence: number;
}

export type HealthFilter =
	| "all"
	| "healthy"
	| "attention"
	| "degraded"
	| "unknown"
	| "notEvaluated"
	| "restarted";

export interface ScopePill {
	kind: "namespaces" | "kinds" | "argoApp" | "gitOpsOwner";
	label: string;
	value: string;
}

export interface ResourceSearchEntry {
	resource: ResourceSummary;
	searchText: string;
	gitOpsFilterKeys: string[];
}
const SUCCESS_STATUS_VALUES = new Set([
	"running",
	"succeeded",
	"complete",
	"completed",
	"ready",
]);
const FAILURE_STATUS_VALUES = new Set([
	"failed",
	"error",
	"crashloopbackoff",
	"imagepullbackoff",
]);
const WARNING_STATUS_VALUES = new Set(["pending", "terminating", "unknown"]);

function normalized(value: string | undefined): string {
	return value?.trim().toLowerCase() ?? "";
}

export function resourceStatusTone(value: string | undefined): StatusTone {
	const status = normalized(value);
	if (SUCCESS_STATUS_VALUES.has(status)) return "success";
	if (FAILURE_STATUS_VALUES.has(status)) return "error";
	if (WARNING_STATUS_VALUES.has(status)) return "warning";
	return "neutral";
}

function isSuccessfulTerminalPod(
	row: Pick<ResourceSummary, "kind" | "status">,
): boolean {
	return (
		row.kind === "Pod" &&
		["succeeded", "complete", "completed"].includes(normalized(row.status))
	);
}

export function resourceReadyChip(
	row: Pick<ResourceSummary, "kind" | "status" | "ready">,
): { value: string; tone: StatusTone } | null {
	const ready = normalized(row.ready);
	if (ready === "true") return { value: "Ready", tone: "success" };
	if (ready === "false") {
		return isSuccessfulTerminalPod(row)
			? { value: "Completed", tone: "success" }
			: { value: "Not ready", tone: "error" };
	}
	return null;
}

export function argoApplicationResourceNamespaces(
	app: Pick<ArgoApplicationSummary, "destinationNamespace" | "resourceNamespaces">,
): string[] {
	const namespaces = Array.from(
		new Set(app.resourceNamespaces.map((namespace) => namespace.trim()).filter(Boolean)),
	).sort((a, b) => a.localeCompare(b));
	if (namespaces.length > 0) return namespaces;
	const destination = app.destinationNamespace?.trim();
	return destination ? [destination] : [];
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
	"EndpointSlice",
	"Ingress",
	"ConfigMap",
	"Secret",
] as const;

export function resourceSelectionKey(resource: ResourceSummary): string {
	return `${resource.cluster}:${resource.apiVersion ?? ""}:${resource.kind}:${resource.namespace ?? ""}:${resource.name}`;
}

export function resourceIdentityKey(resource: ResourceSummary): string {
	return `${resource.cluster}:${resource.kind}:${resource.namespace ?? ""}:${resource.name}`;
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
		if (namespaces.length === 0) {
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
	const coalesced = new Map<string, WatchResourceKey>();
	for (const key of keys) {
		if (isDiscoveredResourceKind(key.kind)) {
			const watchKey = {
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
			coalesced.set(watchKeySignature(watchKey), watchKey);
			continue;
		}

		const watchKey = {
			resourceKind: { kind: key.kind },
			namespace: key.namespace,
		};
		coalesced.set(watchKeySignature(watchKey), watchKey);
	}
	return Array.from(coalesced.values());
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

export function shouldDropWarmupWatchEvent(action: string, elapsedMs: number): boolean {
	return action === "added" && elapsedMs < 2_000;
}

export function topologyWatchKeys(namespaces: string[]): WatchResourceKey[] {
	const namespaceScopes: Array<string | undefined> =
		namespaces.length === 0 ? [undefined] : namespaces;
	return TOPOLOGY_WATCH_KINDS.flatMap((kind) =>
		namespaceScopes.map((namespace) => ({
			resourceKind: { kind },
			namespace,
		})),
	);
}

export function sortedRows(
	data: ResourceSummary[],
	sorting: ResourceSortingState,
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
	return filterResourceSearchIndex(
		buildResourceSearchIndex(data),
		search,
		argoAppFilter,
	);
}

export function buildResourceSearchIndex(
	data: ResourceSummary[],
): ResourceSearchEntry[] {
	return data.map((resource) => {
		const ownership = gitOpsOwnership(resource);
		return {
			resource,
			gitOpsFilterKeys: ownership?.filterKeys ?? [],
			searchText: [
				resource.name,
				resource.namespace,
				resource.kind,
				resource.apiVersion,
				resource.group,
				resource.plural,
				resource.ownerRef,
				resource.helmRelease,
				...(ownership?.searchTerms ?? []),
			]
				.filter((value): value is string => Boolean(value))
				.join("\n")
				.toLowerCase(),
		};
	});
}

export function filterResourceSearchIndex(
	index: ResourceSearchEntry[],
	search: string,
	argoAppFilter: string,
): ResourceSummary[] {
	const term = search.trim().toLowerCase();
	const rows: ResourceSummary[] = [];
	for (const entry of index) {
		if (argoAppFilter && !entry.gitOpsFilterKeys.includes(argoAppFilter)) {
			continue;
		}
		if (!term || entry.searchText.includes(term)) {
			rows.push(entry.resource);
		}
	}
	return rows;
}

export function formatResourceTypeGroupLabel(resource: ResourceSummary): string {
	const plural = resource.plural?.trim();
	if (plural) {
		const kindLower = resource.kind.toLowerCase();
		const pluralLower = plural.toLowerCase();
		let sharedPrefix = 0;
		while (
			sharedPrefix < kindLower.length &&
			sharedPrefix < pluralLower.length &&
			kindLower[sharedPrefix] === pluralLower[sharedPrefix]
		) {
			sharedPrefix += 1;
		}
		return sharedPrefix > 0
			? `${resource.kind.slice(0, sharedPrefix)}${plural.slice(sharedPrefix)}`
			: `${plural[0]?.toUpperCase() ?? ""}${plural.slice(1)}`;
	}
	if (/[^aeiou]y$/i.test(resource.kind)) return `${resource.kind.slice(0, -1)}ies`;
	if (/(s|x|z|ch|sh)$/i.test(resource.kind)) return `${resource.kind}es`;
	return `${resource.kind}s`;
}

const RESOURCE_GROUP_KIND_RANK: Record<string, number> = {
	Deployment: 10,
	StatefulSet: 11,
	DaemonSet: 12,
	ReplicaSet: 13,
	Pod: 20,
	Job: 30,
	CronJob: 31,
	Service: 40,
	Ingress: 41,
	EndpointSlice: 42,
	PersistentVolumeClaim: 50,
	ConfigMap: 80,
	Secret: 81,
};

export function resourceGroupKindRank(kind: string): number {
	return RESOURCE_GROUP_KIND_RANK[kind] ?? 70;
}

export function resourceGroupCollapseKey(resource: ResourceSummary): string {
	return `group:${gitOpsOwnershipGroupLabel(resource)}`;
}

export function resourceTypeGroupCollapseKey(
	resource: ResourceSummary,
): string {
	return `${resourceGroupCollapseKey(resource)}::type:${formatResourceTypeGroupLabel(resource)}`;
}

export function describeResourceScope(
	namespaces: string[],
	kinds: ResourceKindSelection[],
	argoAppFilter: string,
): ScopePill[] {
	const pills: ScopePill[] = [];
	pills.push({
		kind: "namespaces",
		label: namespaces.length === 1 ? "Namespace" : "Namespaces",
		value:
			namespaces.length === 0
				? "All namespaces"
				: namespaces.length <= 2
					? namespaces.join(", ")
					: `${namespaces.slice(0, 2).join(", ")} +${namespaces.length - 2}`,
	});
	if (kinds.length > 0) {
		pills.push({
			kind: "kinds",
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
		pills.push({
			kind: "gitOpsOwner",
			label: "GitOps",
			value: gitOpsOwnershipFilterValue(argoAppFilter),
		});
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
				unknown: summary.unknown + (flags.unknown ? 1 : 0),
				notEvaluated: summary.notEvaluated + (flags.notEvaluated ? 1 : 0),
				restartEvidence: summary.restartEvidence + (flags.restarted ? 1 : 0),
			};
		},
		{
			total: 0,
			healthy: 0,
			attention: 0,
			degraded: 0,
			unknown: 0,
			notEvaluated: 0,
			restartEvidence: 0,
		},
	);
}

export function tableTooltipText(
	value: string | number | null | undefined,
): string {
	return value === undefined || value === null || value === ""
		? "—"
		: String(value);
}
