import type {
	DiscoveredResourceKind,
	ResourceKindSelection,
	ResourceSummary,
	TopologyNode,
} from "@/lib/types";
import { CLUSTER_SCOPED_KINDS, SUPPORTED_KINDS } from "@/lib/types";
import { PAGE_SIZE } from "./constants";
import { pageGitOpsGroupCounts, pageTypeGroupCounts } from "./grouping";
import {
	buildResourceHealthSummary,
	buildResourceSearchIndex,
	filterResourceSearchIndex,
	filterResourcesByHealth,
	formatResourceGroupLabel,
	formatResourceTypeGroupLabel,
	type HealthFilter,
	hasResourceListGitOpsOwner,
	type ResourceSearchEntry,
	resourceGroupCollapseKey,
	resourceGroupKindRank,
	resourceIdentityKey,
	resourceKindFetchKey,
	resourceKindLabel,
	resourceSelectionKey,
	resourceTypeGroupCollapseKey,
	uniqueGitOpsFilters,
} from "./helpers";

export type ResourceSortColumn =
	| "name"
	| "namespace"
	| "kind"
	| "status"
	| "ready"
	| "restarts"
	| "age"
	| "cpu"
	| "memory";

export interface ResourceSort {
	id: ResourceSortColumn;
	desc: boolean;
}

export interface ResourceTableState {
	search: string;
	gitOpsFilter: string;
	healthFilter: HealthFilter;
	sort: ResourceSort;
	pageIndex: number;
	collapsedGroups: Set<string>;
	selectedResource?: ResourceSummary | null;
}

export type ResourceTableEntry =
	| {
			type: "group";
			key: string;
			label: string;
			count: number;
			collapsed: boolean;
	  }
	| {
			type: "type";
			key: string;
			label: string;
			kind: string;
			count: number;
			collapsed: boolean;
	  }
	| {
			type: "resource";
			key: string;
			resource: ResourceSummary;
	  };

export interface ResourceTableModel {
	scopedRows: ResourceSummary[];
	filteredRows: ResourceSummary[];
	displayRows: ResourceSummary[];
	pageRows: ResourceSummary[];
	entries: ResourceTableEntry[];
	totalRows: number;
	pageCount: number;
	safePageIndex: number;
	groupedByGitOps: boolean;
	gitOpsFilters: ReturnType<typeof uniqueGitOpsFilters>;
	healthSummary: ReturnType<typeof buildResourceHealthSummary>;
	columnVisibility: {
		ready: boolean;
		restarts: boolean;
		cpu: boolean;
		memory: boolean;
		gitOps: boolean;
	};
}

export function initialOwnershipMapOpen(
	restoredState: { mapPanelOpen: boolean } | null | undefined,
	showByDefault: boolean,
): boolean {
	return restoredState?.mapPanelOpen ?? showByDefault;
}

export function shouldLoadOwnershipMap(
	mapPanelOpen: boolean,
	componentLoaded: boolean,
	loadFailed: boolean,
): boolean {
	return mapPanelOpen && !componentLoaded && !loadFailed;
}

export function kindSelectionKey(kind: ResourceKindSelection): string {
	return resourceKindFetchKey(kind);
}

export function kindSelectionLabel(kind: ResourceKindSelection): string {
	return resourceKindLabel(kind);
}

export interface ResourceScopeOption {
	key: string;
	label: string;
}

export function filterResourceScopeOptions(
	options: ResourceScopeOption[],
	search: string,
): ResourceScopeOption[] {
	const query = search.trim().toLowerCase();
	return query
		? options.filter((option) => option.label.toLowerCase().includes(query))
		: options;
}

export function nextNamespaceSelection(
	selectedNamespaces: string[],
	availableNamespaces: string[],
	namespace: string,
	checked: boolean,
): string[] {
	if (checked) return Array.from(new Set([...selectedNamespaces, namespace])).sort();
	const currentSelection = selectedNamespaces.length > 0 ? selectedNamespaces : availableNamespaces;
	return currentSelection.filter((item) => item !== namespace);
}

export function allKindOptions(
	discoveredKinds: DiscoveredResourceKind[],
): ResourceKindSelection[] {
	const discovered = discoveredKinds
		.toSorted((left, right) => left.kind.localeCompare(right.kind))
	return [...SUPPORTED_KINDS, ...CLUSTER_SCOPED_KINDS, ...discovered];
}

export function syncedTopologyNodeId({
	selectedTopologyNodeId,
	selectedResource,
	topologyNodes,
}: {
	selectedTopologyNodeId: string | null;
	selectedResource: ResourceSummary | null;
	topologyNodes: TopologyNode[] | undefined;
}): string | null {
	if (!topologyNodes) return selectedTopologyNodeId;
	if (
		selectedTopologyNodeId &&
		!topologyNodes.some((node) => node.id === selectedTopologyNodeId)
	) {
		return null;
	}
	if (!selectedResource) return selectedTopologyNodeId;
	const selectedKey = resourceSelectionKey(selectedResource);
	const selectedIdentityKey = resourceIdentityKey(selectedResource);
	const selectedFromTable =
		topologyNodes.find((node) => resourceSelectionKey(node.summary) === selectedKey) ??
		topologyNodes.find(
			(node) => resourceIdentityKey(node.summary) === selectedIdentityKey,
		);
	return selectedFromTable?.id ?? selectedTopologyNodeId;
}

function valueForSort(
	row: ResourceSummary,
	column: ResourceSortColumn,
): string | number {
	switch (column) {
		case "namespace":
			return row.namespace ?? "";
		case "kind":
			return row.kind;
		case "status":
			return row.status ?? row.health;
		case "ready":
			return row.ready ?? "";
		case "restarts":
			return row.restarts ?? -1;
		case "age":
			return row.createdAt ?? row.age;
		case "cpu":
			return row.metrics?.cpuMillicores ?? -1;
		case "memory":
			return row.metrics?.memoryBytes ?? -1;
		default:
			return row.name;
	}
}

function compareValues(left: string | number, right: string | number): number {
	if (typeof left === "number" && typeof right === "number") {
		return left - right;
	}
	return String(left).localeCompare(String(right), undefined, {
		numeric: true,
		sensitivity: "base",
	});
}

function sortedResourceRows(
	rows: ResourceSummary[],
	sort: ResourceSort,
): ResourceSummary[] {
	return [...rows].sort((left, right) => {
		const compare = compareValues(
			valueForSort(left, sort.id),
			valueForSort(right, sort.id),
		);
		if (compare !== 0) return sort.desc ? -compare : compare;
		return left.name.localeCompare(right.name);
	});
}

function gitOpsGroupedRows(rows: ResourceSummary[]): ResourceSummary[] {
	return rows.toSorted((left, right) => {
		const groupCompare = formatResourceGroupLabel(left).localeCompare(
			formatResourceGroupLabel(right),
		);
		if (groupCompare !== 0) return groupCompare;
		const rankCompare =
			resourceGroupKindRank(left.kind) - resourceGroupKindRank(right.kind);
		if (rankCompare !== 0) return rankCompare;
		const kindCompare = left.kind.localeCompare(right.kind);
		if (kindCompare !== 0) return kindCompare;
		return left.name.localeCompare(right.name);
	});
}

function typeGroupedRows(rows: ResourceSummary[]): ResourceSummary[] {
	return rows.toSorted((left, right) => {
		const rankCompare =
			resourceGroupKindRank(left.kind) - resourceGroupKindRank(right.kind);
		if (rankCompare !== 0) return rankCompare;
		const kindCompare = left.kind.localeCompare(right.kind);
		if (kindCompare !== 0) return kindCompare;
		return 0;
	});
}

function ownerLookupKey(resource: ResourceSummary): string {
	return `${resource.namespace ?? ""}/${resource.name}`;
}

function resourcesWithInheritedGitOpsOwners(
	rows: ResourceSummary[],
): ResourceSummary[] {
	const ownerKeys = new Set<string>();
	for (const row of rows) {
		if (row.ownerRef) ownerKeys.add(`${row.namespace ?? ""}/${row.ownerRef}`);
	}
	if (ownerKeys.size === 0) return rows;

	const resourcesByName = new Map<string, ResourceSummary>();
	for (const row of rows) {
		const key = ownerLookupKey(row);
		if (ownerKeys.has(key)) resourcesByName.set(key, row);
	}
	if (resourcesByName.size === 0) return rows;
	const ownerCache = new Map<ResourceSummary, ResourceSummary | null>();

	const owningResource = (resource: ResourceSummary): ResourceSummary | null => {
		if (ownerCache.has(resource)) return ownerCache.get(resource) ?? null;
		if (hasResourceListGitOpsOwner(resource)) return resource;
		const seen = new Set<ResourceSummary>([resource]);
		let current: ResourceSummary | undefined = resource;
		while (current?.ownerRef) {
			const owner = resourcesByName.get(`${current.namespace ?? ""}/${current.ownerRef}`);
			if (!owner || seen.has(owner)) break;
			if (hasResourceListGitOpsOwner(owner)) {
				ownerCache.set(resource, owner);
				return owner;
			}
			seen.add(owner);
			current = owner;
		}
		ownerCache.set(resource, null);
		return null;
	};

	return rows.map((row) => {
		if (hasResourceListGitOpsOwner(row)) return row;
		const owner = owningResource(row);
		if (!owner) return row;
		return {
			...row,
			argoApp: row.argoApp ?? owner.argoApp,
			gitOpsOwner: row.gitOpsOwner ?? owner.gitOpsOwner,
		};
	});
}

function buildEntries({
	pageRows,
	groupedByGitOps,
	collapsedGroups,
}: {
	pageRows: ResourceSummary[];
	groupedByGitOps: boolean;
	collapsedGroups: Set<string>;
}): ResourceTableEntry[] {
	const pageGroups = groupedByGitOps
		? pageGitOpsGroupCounts(pageRows, groupedByGitOps)
		: new Map<string, number>();
	const pageTypeGroups = pageTypeGroupCounts(pageRows, true);
	const entries: ResourceTableEntry[] = [];
	let previous: ResourceSummary | null = null;
	for (const resource of pageRows) {
		const groupLabel = formatResourceGroupLabel(resource);
		const typeLabel = formatResourceTypeGroupLabel(resource);
		const groupKey = resourceGroupCollapseKey(resource);
		const typeKey = resourceTypeGroupCollapseKey(resource);
		const groupCollapsed = groupedByGitOps && collapsedGroups.has(groupKey);
		const typeCollapsed = collapsedGroups.has(typeKey);
		const showGroup =
			groupedByGitOps &&
			(!previous || formatResourceGroupLabel(previous) !== groupLabel);
		const showType =
			!previous ||
			(groupedByGitOps && formatResourceGroupLabel(previous) !== groupLabel) ||
			formatResourceTypeGroupLabel(previous) !== typeLabel;
		if (showGroup) {
			entries.push({
				type: "group",
				key: groupKey,
				label: groupLabel,
				count: pageGroups.get(groupLabel) ?? 0,
				collapsed: groupCollapsed,
			});
		}
		if (showType && !groupCollapsed) {
			entries.push({
				type: "type",
				key: typeKey,
				label: typeLabel,
				kind: resource.kind,
				count: pageTypeGroups.get(`${groupLabel}::${typeLabel}`) ?? 0,
				collapsed: typeCollapsed,
			});
		}
		if (!groupCollapsed && !typeCollapsed) {
			entries.push({
				type: "resource",
				key: resourceSelectionKey(resource),
				resource,
			});
		}
		previous = resource;
	}
	return entries;
}

function effectiveCollapsedGroups({
	collapsedGroups,
	displayRows,
	selectedResource,
}: {
	collapsedGroups: Set<string>;
	displayRows: ResourceSummary[];
	selectedResource?: ResourceSummary | null;
}): Set<string> {
	if (!selectedResource) return collapsedGroups;
	const selectedKey = resourceSelectionKey(selectedResource);
	const selectedIdentityKey = resourceIdentityKey(selectedResource);
	const selectedRow = displayRows.find(
		(resource) =>
			resourceSelectionKey(resource) === selectedKey ||
			resourceIdentityKey(resource) === selectedIdentityKey,
	);
	if (!selectedRow) return collapsedGroups;
	const groupKey = resourceGroupCollapseKey(selectedRow);
	const typeKey = resourceTypeGroupCollapseKey(selectedRow);
	if (!collapsedGroups.has(groupKey) && !collapsedGroups.has(typeKey)) return collapsedGroups;
	const next = new Set(collapsedGroups);
	next.delete(groupKey);
	next.delete(typeKey);
	return next;
}

export function buildResourceTableModel(
	rows: ResourceSummary[],
	state: ResourceTableState,
	searchIndex: ResourceSearchEntry[] = buildResourceSearchIndex(rows),
): ResourceTableModel {
	const scopedRows = filterResourceSearchIndex(
		searchIndex,
		state.search,
		state.gitOpsFilter,
	);
	const filteredRows = resourcesWithInheritedGitOpsOwners(
		filterResourcesByHealth(scopedRows, state.healthFilter),
	);
	const sortedRows = sortedResourceRows(filteredRows, state.sort);
	const groupedByGitOps = filteredRows.some(hasResourceListGitOpsOwner);
	const displayRows = groupedByGitOps
		? gitOpsGroupedRows(sortedRows)
		: typeGroupedRows(sortedRows);
	const visibleCollapsedGroups = effectiveCollapsedGroups({
		collapsedGroups: state.collapsedGroups,
		displayRows,
		selectedResource: state.selectedResource,
	});
	const pageCount = Math.max(1, Math.ceil(displayRows.length / PAGE_SIZE));
	const safePageIndex = Math.min(state.pageIndex, pageCount - 1);
	const pageRows = displayRows.slice(
		safePageIndex * PAGE_SIZE,
		safePageIndex * PAGE_SIZE + PAGE_SIZE,
	);
	return {
		scopedRows,
		filteredRows,
		displayRows,
		pageRows,
		entries: buildEntries({
			pageRows,
			groupedByGitOps,
			collapsedGroups: visibleCollapsedGroups,
		}),
		totalRows: displayRows.length,
		pageCount,
		safePageIndex,
		groupedByGitOps,
		gitOpsFilters: uniqueGitOpsFilters(rows),
		healthSummary: buildResourceHealthSummary(scopedRows),
		columnVisibility: {
			ready: pageRows.some((row) => Boolean(row.ready)),
			restarts: pageRows.some((row) => row.restarts !== undefined),
			cpu: pageRows.some((row) => row.metrics?.cpuMillicores !== undefined),
			memory: pageRows.some((row) => row.metrics?.memoryBytes !== undefined),
			gitOps: pageRows.some(
				(row) =>
					Boolean(row.gitOpsOwner) ||
					Boolean(row.argoApp) ||
					Boolean(row.helmRelease),
			),
		},
	};
}
