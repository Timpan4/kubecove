import { argoResourceIdentityKey } from "@/features/gitops/argo-workspace-model";
import {
	gitOpsOwnership,
	gitOpsOwnershipFilters,
	gitOpsOwnershipGroup,
	gitOpsOwnershipGroupLabel,
	inheritGitOpsOwnership,
} from "@/lib/gitops-ownership-evidence";
import { resourceHealthAssessment } from "@/lib/resource-health";
import { findResourceIndex, resourceKey } from "@/lib/resource-identity";
import { defaultResourceKindLanes, resourceKindOrder } from "@/lib/resource-kind-order";
import type { ResourceSummary } from "@/lib/types";
import { PAGE_SIZE } from "./constants";
import {
	kindGroupRank,
	pageGitOpsGroupCounts,
	pageTypeGroupCounts,
} from "./grouping";
import {
	buildResourceHealthSummary,
	buildResourceSearchIndex,
	filterResourceSearchIndex,
	filterResourcesByHealth,
	formatResourceTypeGroupLabel,
	resourceGroupCollapseKey,
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

const DEFAULT_KIND_ORDER = resourceKindOrder(defaultResourceKindLanes());

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

function kindGroupedRows(
	rows: ResourceSummary[],
	rankForGroup: (firstRow: ResourceSummary) => number,
	out: ResourceSummary[],
): void {
	const groups = new Map<string, { rank: number; rows: ResourceSummary[] }>();
	for (const row of rows) {
		const group = groups.get(row.kind);
		if (group) group.rows.push(row);
		else groups.set(row.kind, { rank: rankForGroup(row), rows: [row] });
	}
	// Stable sort: equal ranks keep the column-sort order of their first row.
	const ranked = Array.from(groups.values()).sort((left, right) => left.rank - right.rank);
	for (const group of ranked) {
		for (const row of group.rows) out.push(row);
	}
}

function gitOpsGroupedRows(
	rows: ResourceSummary[],
	kindOrder: readonly string[],
	preferredGitOpsResourceKeys?: ReadonlySet<string>,
): ResourceSummary[] {
	const isPreferred = (row: ResourceSummary) =>
		isPreferredGitOpsResource(row, preferredGitOpsResourceKeys);
	const preferredRows: ResourceSummary[] = [];
	const otherRows: ResourceSummary[] = [];
	for (const row of rows) (isPreferred(row) ? preferredRows : otherRows).push(row);
	for (const row of otherRows) preferredRows.push(row);
	const ownershipGroups = new Map<string, { rank: number; rows: ResourceSummary[] }>();
	for (const row of preferredRows) {
		const { label, rank } = gitOpsOwnershipGroup(row);
		const group = ownershipGroups.get(label);
		if (group) group.rows.push(row);
		else ownershipGroups.set(label, { rank: isPreferred(row) ? -1 : rank, rows: [row] });
	}
	const rankedOwnershipGroups = Array.from(ownershipGroups.values()).sort(
		(left, right) => left.rank - right.rank,
	);
	const grouped: ResourceSummary[] = [];
	for (const { rank, rows: ownershipRows } of rankedOwnershipGroups) {
		// Only a group led by a preferred row can contain preferred rows to lift.
		kindGroupedRows(
			ownershipRows,
			(row) => (rank === -1 && isPreferred(row) ? -1 : kindGroupRank(row, kindOrder)),
			grouped,
		);
	}
	return grouped;
}

function isPreferredGitOpsResource(
	row: ResourceSummary,
	preferredKeys?: ReadonlySet<string>,
): boolean {
	if (!preferredKeys?.size) return false;
	const key = argoResourceIdentityKey(row);
	return key !== null && preferredKeys.has(key);
}

function typeGroupedRows(
	rows: ResourceSummary[],
	kindOrder: readonly string[],
): ResourceSummary[] {
	const grouped: ResourceSummary[] = [];
	kindGroupedRows(rows, (row) => kindGroupRank(row, kindOrder), grouped);
	return grouped;
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
				key: resourceKey(resource),
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
	const selectedRow =
		displayRows[findResourceIndex(displayRows, selectedResource, (row) => row)];
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
	const kindOrder = state.kindOrder ?? DEFAULT_KIND_ORDER;
	const displayRows = groupedByGitOps
		? gitOpsGroupedRows(sortedRows, kindOrder, state.preferredGitOpsResourceKeys)
		: typeGroupedRows(sortedRows, kindOrder);
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
