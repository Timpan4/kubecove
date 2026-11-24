import assert from "node:assert/strict";
import { describe } from "vitest";
import {
	filterHistoricalReplicaSets,
	filterTopologyByTableRows,
	syncedTopologyNodeId,
} from "@/features/resources/resourceBrowserModel";
import type { ResourceTableState } from "@/features/resources/resourceTableModel";
import {
	buildResourceTableModel,
	buildResourceTableProjection,
} from "@/features/resources/resourceTableModel";
import { buildFlowTopologyFitPlan } from "@/features/resources/topology-implementation";
import {
	buildFlowTopologyLayoutState,
	buildFlowTopologyViewFromLayoutState,
} from "@/features/resources/topology-view";
import { benchmark } from "./benchmark";
import { buildReplicaSetTopology } from "./topology-benchmark-fixtures";

const baseTopology = buildReplicaSetTopology(500);
const firstPod = baseTopology.nodes.find((node) => node.kind === "Pod");
const firstOwnedByPod = baseTopology.edges.find(
	(edge) => edge.target === firstPod?.id && edge.relation === "owns",
);
const firstDeployment = firstOwnedByPod
	? baseTopology.edges.find(
			(edge) =>
				edge.target === firstOwnedByPod.source && edge.relation === "owns",
			)?.source
	: undefined;
if (!firstPod || !firstOwnedByPod || !firstDeployment)
	throw new Error("Missing branched table-to-map fixture nodes");
const topology = {
	...baseTopology,
	warnings: ["fixture warning is preserved during projection"],
	edges: [
		...baseTopology.edges,
		{
			id: "fixture-cycle",
			source: firstPod.id,
			target: firstDeployment,
			relation: "owns" as const,
		},
		{
			id: "fixture-missing-source",
			source: "missing-topology-node",
			target: firstPod.id,
			relation: "owns" as const,
		},
	],
};
const resources = topology.nodes.map((node) => node.summary);
const projection = buildResourceTableProjection(resources);
const tableState: ResourceTableState = {
	search: "",
	gitOpsFilter: "",
	healthFilter: "all",
	sort: { id: "name", desc: false },
	pageIndex: 0,
	collapsedGroups: new Set(),
	selectedResource: resources[42],
};
const tableModel = buildResourceTableModel(projection, tableState);
const historical = filterHistoricalReplicaSets(topology, true);
const mappedTopology = filterTopologyByTableRows(
	historical,
	tableModel.filteredRows,
);
const leafOnlyTopology = filterTopologyByTableRows(
	filterHistoricalReplicaSets(topology, true),
	[firstPod.summary],
);
const emptyTableTopology = filterTopologyByTableRows(topology, []);
if (!mappedTopology) throw new Error("Missing projected topology fixture");
const layoutState = buildFlowTopologyLayoutState(
	mappedTopology,
	"ownership",
	new Set(),
	null,
);
const syncedSelection = syncedTopologyNodeId({
	selectedTopologyNodeId: null,
	selectedResource: tableState.selectedResource ?? null,
	topologyNodes: mappedTopology?.nodes,
});
const view = buildFlowTopologyViewFromLayoutState(layoutState, {
	mode: "ownership",
	selectedNodeId: syncedSelection,
	showFullTopologyOnSelection: false,
	expandedStandaloneKinds: new Set(),
	viewportSize: { width: 1280, height: 800 },
});
const fitPlan = buildFlowTopologyFitPlan(
	view.graph.nodes,
	view.graph.edges,
	syncedSelection,
	"viewport-1280x800",
	{ width: 1280, height: 800 },
);

assert.equal(resources.length, 2_000);
assert.equal(baseTopology.nodes.length, 2_000);
assert.equal(topology.edges.length, baseTopology.edges.length + 2);
assert.equal(tableModel.filteredRows.length, resources.length);
assert(mappedTopology?.nodes.some((node) => node.id === syncedSelection));
assert(leafOnlyTopology?.nodes.some((node) => node.id === firstPod.id));
assert(leafOnlyTopology?.nodes.some((node) => node.id === firstOwnedByPod.source));
assert(leafOnlyTopology?.nodes.some((node) => node.id === firstDeployment));
assert.equal(
	leafOnlyTopology?.nodes.some(
		(node) => node.kind === "ReplicaSet" && node.deploymentRevision === 1,
	),
	false,
);
assert.equal(emptyTableTopology?.nodes.length, 0);
assert.equal(emptyTableTopology?.edges.length, 0);
assert.equal(leafOnlyTopology?.edges.some((edge) => edge.id === "fixture-cycle"), true);
assert.equal(
	leafOnlyTopology?.edges.some((edge) => edge.id === "fixture-missing-source"),
	false,
);
assert.deepEqual(leafOnlyTopology?.warnings, topology.warnings);
assert.equal(filterTopologyByTableRows(undefined, []), undefined);
assert(view.graph.nodes.some((node) => node.data.selected));
assert(fitPlan && Number.isFinite(fitPlan.viewport.zoom));

function updateTableToMap(state: ResourceTableState) {
	const nextTable = buildResourceTableModel(projection, state);
	const nextHistorical = filterHistoricalReplicaSets(topology, true);
	const nextMapped = filterTopologyByTableRows(
		nextHistorical,
		nextTable.filteredRows,
	);
	if (!nextMapped) return null;
	const nextLayout = buildFlowTopologyLayoutState(
		nextMapped,
		"ownership",
		new Set(),
		null,
	);
	const nextSelection = syncedTopologyNodeId({
		selectedTopologyNodeId: syncedSelection,
		selectedResource: state.selectedResource ?? null,
		topologyNodes: nextMapped?.nodes,
	});
	const nextView = buildFlowTopologyViewFromLayoutState(nextLayout, {
		mode: "ownership",
		selectedNodeId: nextSelection,
		showFullTopologyOnSelection: false,
		expandedStandaloneKinds: new Set(),
		viewportSize: { width: 1280, height: 800 },
	});
	return buildFlowTopologyFitPlan(
		nextView.graph.nodes,
		nextView.graph.edges,
		nextSelection,
		"viewport-1280x800",
		{ width: 1280, height: 800 },
	);
}

const tableUpdates: ResourceTableState[] = [
	tableState,
	{ ...tableState, pageIndex: 1 },
	{ ...tableState, selectedResource: resources[43] },
	{ ...tableState, search: "api-4", selectedResource: resources[44] },
];
const updatedPlans = tableUpdates.map(updateTableToMap);
assert(
	updatedPlans.every(
		(plan) => plan === null || Number.isFinite(plan.viewport.zoom),
	),
);

describe("F06 table-to-map updates with folded F03 selection reconciliation", () => {
	benchmark("F06 table to consumed topology and fit plan without metric enrichment", () => {
		for (const state of tableUpdates) updateTableToMap(state);
		filterTopologyByTableRows(
			filterHistoricalReplicaSets(topology, true),
			[firstPod.summary],
		);
		filterTopologyByTableRows(topology, []);
	});

	benchmark("F03 selection reconciliation for retained table updates", () => {
		for (const state of tableUpdates) {
			syncedTopologyNodeId({
				selectedTopologyNodeId: syncedSelection,
				selectedResource: state.selectedResource ?? null,
				topologyNodes: mappedTopology?.nodes,
			});
		}
	});
});
