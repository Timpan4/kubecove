import { argoResourceIdentityKey } from "@/features/gitops/argo-workspace-model";
import {
	gitOpsOwnership,
	gitOpsOwnershipFilters,
	gitOpsOwnershipGroupLabel,
	inheritGitOpsOwnership,
} from "@/lib/gitops-ownership-evidence";
import { resourceHealthAssessment } from "@/lib/resource-health";
import type { ResourceSummary } from "@/lib/types";
import { PAGE_SIZE } from "./constants";
import { pageGitOpsGroupCounts, pageTypeGroupCounts } from "./grouping";
import {
	buildResourceHealthSummary,
	buildResourceSearchIndex,
	filterResourceSearchIndex,
	filterResourcesByHealth,
	formatResourceTypeGroupLabel,
	resourceGroupCollapseKey,
	resourceIdentityKey,
	resourceSelectionKey,
	resourceTypeGroupCollapseKey,
} from "./helpers";
import type {
	ResourceSort,
	ResourceSortColumn,
	ResourceTableEntry,
	ResourceTableModel,
	ResourceTableProjection,
	ResourceTableState,
} from "./resourceTableTypes";

export type {
	ResourceSort,
	ResourceSortColumn,
	ResourceTableEntry,
	ResourceTableModel,
	ResourceTableProjection,
	ResourceTableState,
} from "./resourceTableTypes";

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
			return resourceHealthAssessment(row)?.state ?? "assessmentUnavailable";
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
	if (isNumber(left) && isNumber(right)) {
		return left - right;
	}
	return String(left).localeCompare(String(right), undefined, {
		numeric: true,
		sensitivity: "base",
	});
}

function isNumber<Value>(value: Value): value is Value & number {
	return Number(value) === value;
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

function groupedRows(
	rows: ResourceSummary[],
	keyForRow: (row: ResourceSummary) => string,
): ResourceSummary[] {
	const groups = new Map<string, ResourceSummary[]>();
	for (const row of rows) {
		const key = keyForRow(row);
		const group = groups.get(key);
		if (group) group.push(row);
		else groups.set(key, [row]);
	}
	return Array.from(groups.values()).flat();
}

function gitOpsGroupedRows(
	rows: ResourceSummary[],
	preferredGitOpsResourceKeys?: ReadonlySet<string>,
): ResourceSummary[] {
	const preferredRows = rows.toSorted((left, right) => {
		const preferredPriority =
			Number(isPreferredGitOpsResource(left, preferredGitOpsResourceKeys)) -
			Number(isPreferredGitOpsResource(right, preferredGitOpsResourceKeys));
		return -preferredPriority;
	});
	const ownershipGroups = new Map<string, ResourceSummary[]>();
	for (const row of preferredRows) {
		const key = gitOpsOwnershipGroupLabel(row);
		const group = ownershipGroups.get(key);
		if (group) group.push(row);
		else ownershipGroups.set(key, [row]);
	}
	const grouped: ResourceSummary[] = [];
	for (const ownershipRows of ownershipGroups.values()) {
		grouped.push(...groupedRows(ownershipRows, (row) => row.kind));
	}
	return grouped;
}

function isPreferredGitOpsResource(
	row: ResourceSummary,
	preferredKeys?: ReadonlySet<string>,
): boolean {
	const key = argoResourceIdentityKey(row);
	return key !== null && preferredKeys?.has(key) === true;
}

function typeGroupedRows(rows: ResourceSummary[]): ResourceSummary[] {
	return groupedRows(rows, (row) => row.kind);
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
		const groupLabel = gitOpsOwnershipGroupLabel(resource);
		const typeLabel = formatResourceTypeGroupLabel(resource);
		const groupKey = resourceGroupCollapseKey(resource);
		const typeKey = resourceTypeGroupCollapseKey(resource);
		const groupCollapsed = groupedByGitOps && collapsedGroups.has(groupKey);
		const typeCollapsed = collapsedGroups.has(typeKey);
		const showGroup =
			groupedByGitOps &&
			(!previous || gitOpsOwnershipGroupLabel(previous) !== groupLabel);
		const showType =
			!previous ||
			(groupedByGitOps && gitOpsOwnershipGroupLabel(previous) !== groupLabel) ||
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

export function buildResourceTableProjection(
	rows: ResourceSummary[],
): ResourceTableProjection {
	const inheritedRows = inheritGitOpsOwnership(rows);
	return {
		searchIndex: buildResourceSearchIndex(inheritedRows),
		gitOpsFilters: gitOpsOwnershipFilters(inheritedRows),
	};
}

export function buildResourceTableModel(
	projection: ResourceTableProjection,
	state: ResourceTableState,
): ResourceTableModel {
	const scopedRows = filterResourceSearchIndex(
		projection.searchIndex,
		state.search,
		state.gitOpsFilter,
	);
	const filteredRows = filterResourcesByHealth(scopedRows, state.healthFilter);
	const sortedRows = sortedResourceRows(filteredRows, state.sort);
	const groupedByGitOps = filteredRows.some((row) => gitOpsOwnership(row) !== null);
	const displayRows = groupedByGitOps
		? gitOpsGroupedRows(sortedRows, state.preferredGitOpsResourceKeys)
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
		gitOpsFilters: projection.gitOpsFilters,
		healthSummary: buildResourceHealthSummary(scopedRows),
		columnVisibility: {
			ready: pageRows.some((row) => Boolean(row.ready)),
			restarts: pageRows.some((row) => row.restarts !== undefined),
			cpu: pageRows.some((row) => row.metrics?.cpuMillicores !== undefined),
			memory: pageRows.some((row) => row.metrics?.memoryBytes !== undefined),
			gitOps: pageRows.some((row) => gitOpsOwnership(row) !== null),
		},
	};
}
