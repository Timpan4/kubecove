import { expect, test } from "bun:test";
import { resourceHealthAssessment } from "@/lib/resource-health";
import type { ResourceSummary } from "@/lib/types";
import { PAGE_SIZE } from "./constants";
import { resourceTypeGroupCollapseKey } from "./helpers";
import {
	buildResourceTableModel,
	buildResourceTableProjection,
	type ResourceSortColumn,
	type ResourceTableState,
} from "./resourceTableModel";

// Failure modes: collation options or tie order drift, unstable equal keys,
// missing metrics changing order, and off-page selection failing to expand a group.
const rows: ResourceSummary[] = [
	"pod-10",
	"Pod-2",
	"pod-2",
	"same",
	"same",
	"a",
	"Z",
].map((name, index) => ({
	cluster: "dev",
	kind: "Pod",
	name,
	age: `${index % 3}m`,
	namespace: index % 2 ? "Team-10" : null,
	health: "healthy",
	ready: index % 2 ? "1/1" : undefined,
	restarts: index % 3 ? index : undefined,
	metrics:
		index % 2
			? {
					cluster: "dev",
					kind: "Pod",
					name,
					namespace: null,
					cpuMillicores: index,
					memoryBytes: index * 1024,
					sourcePods: [],
				}
			: undefined,
}));
const state: ResourceTableState = {
	search: "",
	gitOpsFilter: "",
	healthFilter: "all",
	sort: { id: "name", desc: false },
	pageIndex: 0,
	collapsedGroups: new Set(),
};
function legacyValue(
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
test("every sort preserves legacy collation, missing values and stable ties", () => {
	const columns: ResourceSortColumn[] = [
		"name",
		"namespace",
		"kind",
		"status",
		"ready",
		"restarts",
		"age",
		"cpu",
		"memory",
	];
	const projection = buildResourceTableProjection(rows);
	for (const id of columns) {
		for (const desc of [false, true]) {
			const expected = [...rows].sort((a, b) => {
				const left = legacyValue(a, id),
					right = legacyValue(b, id);
				const compared =
					typeof left === "number" && typeof right === "number"
						? left - right
						: String(left).localeCompare(String(right), undefined, {
								numeric: true,
								sensitivity: "base",
							});
				return compared !== 0
					? desc
						? -compared
						: compared
					: a.name.localeCompare(b.name);
			});
			const actual = buildResourceTableModel(projection, {
				...state,
				sort: { id, desc },
			}).displayRows;
			expect(actual).toEqual(expected);
			for (let index = 0; index < actual.length; index++)
				expect(actual[index]).toBe(expected[index]);
		}
	}
});

test("off-page selection expands its group without changing the page or saved collapse state", () => {
	const many = Array.from({ length: PAGE_SIZE + 1 }, (_, index) => ({
		...rows[0],
		name: `pod-${index}`,
	}));
	const collapsedGroups = new Set([resourceTypeGroupCollapseKey(many[0])]);
	const projection = buildResourceTableProjection(many);
	const collapsed = buildResourceTableModel(projection, {
		...state,
		collapsedGroups,
	});
	expect(collapsed.entries.some((entry) => entry.type === "resource")).toBe(
		false,
	);
	const expanded = buildResourceTableModel(projection, {
		...state,
		collapsedGroups,
		selectedResource: many.at(-1),
	});
	expect(expanded.safePageIndex).toBe(0);
	expect(
		expanded.entries.filter((entry) => entry.type === "resource"),
	).toHaveLength(PAGE_SIZE);
	expect(collapsedGroups.size).toBe(1);
	const filteredOut = buildResourceTableModel(projection, {
		...state,
		collapsedGroups,
		search: "pod-0",
		selectedResource: many.at(-1),
	});
	expect(filteredOut.entries.some((entry) => entry.type === "resource")).toBe(
		false,
	);
});
