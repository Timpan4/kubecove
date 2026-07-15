import type {
	ArgoApplicationSummary,
	ClusterScopedKind,
	DiscoveredResourceKind,
	ResourceKindSelection,
	ResourceSummary,
	WatchResourceKey,
} from "@/lib/types";
import type { StatusTone } from "@/components/status-badge-styles";
import { classifyResourceHealth } from "@/lib/resource-health";
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
	restarted: number;
	/** Rows with no health semantics (ConfigMaps, Services, …). */
	untracked: number;
}

export type HealthFilter =
	| "all"
	| "healthy"
	| "unhealthy"
	| "attention"
	| "degraded"
	| "restarted";

export interface ScopePill {
	kind: "namespaces" | "kinds" | "argoApp" | "gitOpsOwner";
	label: string;
	value: string;
}

export interface ResourceSearchEntry {
	resource: ResourceSummary;
	searchText: string;
	argoApp: string;
	gitOpsFilterKey: string;
}

export interface GitOpsFilterOption {
	key: string;
	label: string;
}

const LIST_OWNING_FLUX_KINDS = new Set(["Kustomization", "HelmRelease"]);
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

export function argoApplicationGitOpsFilterKey(name: string): string {
	return ["argo", "Application", "", name].join(":");
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

function argoApplicationNameFromGitOpsFilter(filter: string): string | null {
	const [provider, kind, , name] = filter.split(":");
	if (provider !== "argo" || kind !== "Application" || !name) return null;
	return name;
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
	return data.map((resource) => ({
		resource,
		argoApp: resource.argoApp ?? "",
		gitOpsFilterKey: gitOpsFilterKey(resource),
		searchText: [
			resource.name,
			resource.namespace,
			resource.kind,
			resource.apiVersion,
			resource.group,
			resource.plural,
			resource.ownerRef,
			resource.argoApp,
			resource.gitOpsOwner?.provider,
			resource.gitOpsOwner?.kind,
			resource.gitOpsOwner?.name,
			resource.gitOpsOwner?.namespace,
			resource.helmRelease,
		]
			.filter((value): value is string => Boolean(value))
			.join("\n")
			.toLowerCase(),
	}));
}

export function filterResourceSearchIndex(
	index: ResourceSearchEntry[],
	search: string,
	argoAppFilter: string,
): ResourceSummary[] {
	const term = search.trim().toLowerCase();
	const rows: ResourceSummary[] = [];
	for (const entry of index) {
		if (argoAppFilter) {
			const argoApplicationName =
				argoApplicationNameFromGitOpsFilter(argoAppFilter);
			const matchesLegacyArgo =
				entry.argoApp === argoAppFilter ||
				(argoApplicationName !== null && entry.argoApp === argoApplicationName);
			const matchesGitOpsOwner = entry.gitOpsFilterKey === argoAppFilter;
			if (!matchesLegacyArgo && !matchesGitOpsOwner) continue;
		}
		if (!term || entry.searchText.includes(term)) {
			rows.push(entry.resource);
		}
	}
	return rows;
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

export function gitOpsFilterKey(resource: ResourceSummary): string {
	const owner = resource.gitOpsOwner;
	if (owner?.provider === "argo" && owner.kind === "Application") {
		return [
			owner.provider,
			owner.kind,
			owner.namespace ?? "",
			owner.name,
		].join(":");
	}
	if (owner?.provider === "flux" && LIST_OWNING_FLUX_KINDS.has(owner.kind)) {
		return [
			owner.provider,
			owner.kind,
			owner.namespace ?? "",
			owner.name,
		].join(":");
	}
	return resource.argoApp ?? "";
}

export function gitOpsOwnerLabel(resource: ResourceSummary): string {
	const owner = resource.gitOpsOwner;
	if (owner?.provider === "flux" && LIST_OWNING_FLUX_KINDS.has(owner.kind)) {
		const scopedName = owner.namespace
			? `${owner.namespace}/${owner.name}`
			: owner.name;
		return `Owned by Flux ${owner.kind}: ${scopedName}`;
	}
	if (owner?.provider === "argo" && owner.kind === "Application") {
		return `Owned by Argo CD: ${owner.name}`;
	}
	if (resource.argoApp) return `Owned by Argo CD: ${resource.argoApp}`;
	return "";
}

export function hasResourceListGitOpsOwner(resource: ResourceSummary): boolean {
	return Boolean(gitOpsOwnerLabel(resource));
}

export function uniqueGitOpsFilters(
	data: ResourceSummary[],
): GitOpsFilterOption[] {
	const filters = new Map<string, string>();
	for (const resource of data) {
		const key = gitOpsFilterKey(resource);
		if (!key) continue;
		filters.set(key, gitOpsOwnerLabel(resource) || key);
	}
	return Array.from(filters, ([key, label]) => ({ key, label })).sort((a, b) =>
		a.label.localeCompare(b.label),
	);
}

export function formatResourceGroupLabel(resource: ResourceSummary): string {
	const gitOpsLabel = gitOpsOwnerLabel(resource);
	if (gitOpsLabel) return gitOpsLabel;
	return "Unmanaged resources";
}

export function formatResourceTypeGroupLabel(resource: ResourceSummary): string {
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
	return `group:${formatResourceGroupLabel(resource)}`;
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
			value:
				argoApplicationNameFromGitOpsFilter(argoAppFilter) ??
				argoAppFilter,
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

			const untracked = row.health === "unknown";
			return {
				total: summary.total + 1,
				healthy: summary.healthy + (flags.healthy ? 1 : 0),
				attention: summary.attention + (flags.attention ? 1 : 0),
				degraded: summary.degraded + (flags.degraded ? 1 : 0),
				restarted: summary.restarted + (flags.restarted ? 1 : 0),
				untracked: summary.untracked + (untracked ? 1 : 0),
			};
		},
		{ total: 0, healthy: 0, attention: 0, degraded: 0, restarted: 0, untracked: 0 },
	);
}

export function tableTooltipText(
	value: string | number | null | undefined,
): string {
	return value === undefined || value === null || value === ""
		? "—"
		: String(value);
}
