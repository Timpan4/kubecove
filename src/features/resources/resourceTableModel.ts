import { argoResourceIdentityKey } from "@/features/gitops/argo-workspace-model";
import {
	gitOpsOwnership,
	gitOpsOwnershipFilters,
	gitOpsOwnershipGroup,
	inheritGitOpsOwnership,
} from "@/lib/gitops-ownership-evidence";
import { resourceHealthAssessment } from "@/lib/resource-health";
import { looseResourceKey, resourceKey } from "@/lib/resource-identity";
import {
	defaultResourceKindLanes,
	resourceKindOrder,
} from "@/lib/resource-kind-order";
import type { ResourceSummary } from "@/lib/types";
import { PAGE_SIZE } from "./constants";
import { kindGroupRank } from "./grouping";
import { buildResourceTableEntries } from "./resourceTableEntries";
import {
	buildResourceHealthSummary,
	buildResourceSearchIndex,
	filterResourceSearchIndex,
	filterResourcesByHealth,
} from "./helpers";
import type {
	ResourceSort,
	ResourceSortColumn,
	ResourceTableDisplay,
	ResourceTableModel,
	ResourceTablePage,
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
const sortCollator = new Intl.Collator(undefined, {
	numeric: true,
	sensitivity: "base",
});
const nameCollator = new Intl.Collator();

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
	return sortCollator.compare(String(left), String(right));
}

function isNumber<Value>(value: Value): value is Value & number {
	return Number(value) === value;
}

function sortedResourceRows(
	rows: ResourceSummary[],
	sort: ResourceSort,
): ResourceSummary[] {
	const decorated = [];
	for (const row of rows)
		decorated.push({ row, value: valueForSort(row, sort.id) });
	decorated.sort((left, right) => {
		const compare = compareValues(left.value, right.value);
		if (compare !== 0) return sort.desc ? -compare : compare;
		return nameCollator.compare(left.row.name, right.row.name);
	});
	const sorted = [];
	for (const entry of decorated) sorted.push(entry.row);
	return sorted;
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
	const ranked = Array.from(groups.values()).sort(
		(left, right) => left.rank - right.rank,
	);
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
	for (const row of rows)
		(isPreferred(row) ? preferredRows : otherRows).push(row);
	for (const row of otherRows) preferredRows.push(row);
	const ownershipGroups = new Map<
		string,
		{ rank: number; rows: ResourceSummary[] }
	>();
	for (const row of preferredRows) {
		const { label, rank } = gitOpsOwnershipGroup(row);
		const group = ownershipGroups.get(label);
		if (group) group.rows.push(row);
		else
			ownershipGroups.set(label, {
				rank: isPreferred(row) ? -1 : rank,
				rows: [row],
			});
	}
	const rankedOwnershipGroups = Array.from(ownershipGroups.values()).sort(
		(left, right) => left.rank - right.rank,
	);
	const grouped: ResourceSummary[] = [];
	for (const { rank, rows: ownershipRows } of rankedOwnershipGroups) {
		// Only a group led by a preferred row can contain preferred rows to lift.
		kindGroupedRows(
			ownershipRows,
			(row) =>
				rank === -1 && isPreferred(row) ? -1 : kindGroupRank(row, kindOrder),
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

export function buildResourceTableProjection(
	rows: ResourceSummary[],
): ResourceTableProjection {
	const inheritedRows = inheritGitOpsOwnership(rows);
	return {
		searchIndex: buildResourceSearchIndex(inheritedRows),
		gitOpsFilters: gitOpsOwnershipFilters(inheritedRows),
	};
}

export function buildResourceTableDisplay(
	projection: ResourceTableProjection,
	state: Omit<
		ResourceTableState,
		"pageIndex" | "collapsedGroups" | "selectedResource"
	>,
): ResourceTableDisplay {
	const scopedRows = filterResourceSearchIndex(
		projection.searchIndex,
		state.search,
		state.gitOpsFilter,
	);
	const filteredRows = filterResourcesByHealth(scopedRows, state.healthFilter);
	const sortedRows = sortedResourceRows(filteredRows, state.sort);
	const groupedByGitOps = filteredRows.some(
		(row) => gitOpsOwnership(row) !== null,
	);
	const kindOrder = state.kindOrder ?? DEFAULT_KIND_ORDER;
	const displayRows = groupedByGitOps
		? gitOpsGroupedRows(
				sortedRows,
				kindOrder,
				state.preferredGitOpsResourceKeys,
			)
		: typeGroupedRows(sortedRows, kindOrder);
	const pageCount = Math.max(1, Math.ceil(displayRows.length / PAGE_SIZE));
	const exact = new Map<string, ResourceSummary>();
	const loose = new Map<string, ResourceSummary>();
	const unversioned = new Map<string, ResourceSummary>();
	let indexed = false;
	function rowForResource(resource: ResourceSummary) {
		if (!indexed) {
			for (const row of displayRows) {
				const key = resourceKey(row),
					looseKey = looseResourceKey(row);
				if (!exact.has(key)) exact.set(key, row);
				if (!loose.has(looseKey)) loose.set(looseKey, row);
				if (row.apiVersion === undefined && !unversioned.has(looseKey))
					unversioned.set(looseKey, row);
			}
			indexed = true;
		}
		return (
			exact.get(resourceKey(resource)) ??
			(resource.apiVersion === undefined ? loose : unversioned).get(
				looseResourceKey(resource),
			)
		);
	}
	return {
		scopedRows,
		filteredRows,
		displayRows,
		pageCount,
		totalRows: displayRows.length,
		groupedByGitOps,
		gitOpsFilters: projection.gitOpsFilters,
		healthSummary: buildResourceHealthSummary(scopedRows),
		rowForResource,
	};
}

export function buildResourceTablePage(
	display: ResourceTableDisplay,
	pageIndex: number,
): ResourceTablePage {
	const safePageIndex = Math.min(pageIndex, display.pageCount - 1);
	const { displayRows } = display;
	const pageRows = displayRows.slice(
		safePageIndex * PAGE_SIZE,
		safePageIndex * PAGE_SIZE + PAGE_SIZE,
	);
	return {
		...display,
		pageRows,
		safePageIndex,
		columnVisibility: {
			ready: pageRows.some((row) => Boolean(row.ready)),
			restarts: pageRows.some((row) => row.restarts !== undefined),
			cpu: pageRows.some((row) => row.metrics?.cpuMillicores !== undefined),
			memory: pageRows.some((row) => row.metrics?.memoryBytes !== undefined),
			gitOps: pageRows.some((row) => gitOpsOwnership(row) !== null),
		},
	};
}

export function buildResourceTableModel(
	projection: ResourceTableProjection,
	state: ResourceTableState,
): ResourceTableModel {
	const page = buildResourceTablePage(
		buildResourceTableDisplay(projection, state),
		state.pageIndex,
	);
	const { rowForResource, ...model } = page;
	return {
		...model,
		entries: buildResourceTableEntries(
			{ ...model, rowForResource },
			state.collapsedGroups,
			state.selectedResource,
		),
	};
}
