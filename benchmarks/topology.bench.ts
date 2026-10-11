import assert from "node:assert/strict";
import { bench, describe } from "vitest";
import {
	applyFlowTopologySelectionWithIndex,
	buildFlowTopology,
	buildFlowTopologyFitPlan,
	buildFlowTopologyLayout,
	buildFlowTopologySelectionIndex,
	getFlowTopologyBounds,
} from "@/features/resources/topology-implementation";
import {
	buildFlowTopologyLayoutState,
	buildFlowTopologyView,
	buildFlowTopologyViewFromLayoutState,
} from "@/features/resources/topology-view";
import {
	buildBranchingTopology,
	buildOwnershipTopology,
} from "./topology-benchmark-fixtures";

const topology = buildOwnershipTopology(500);
assert.equal(topology.nodes.length, 1_000);
assert.equal(topology.nodes[0]?.kind, "Deployment");
assert.equal(topology.nodes[0]?.name, "api-0");
assert.equal(topology.nodes[1]?.kind, "Pod");
assert.equal(topology.nodes[1]?.name, "api-0-pod");
const selectedIds = topology.nodes
	.filter((node) => node.kind === "Pod")
	.slice(0, 50)
	.map((node) => node.id);
const layout = buildFlowTopologyLayout(topology, null);
const selectionIndex = buildFlowTopologySelectionIndex(topology);
const expandedStandaloneKinds = new Set<string>();
const layoutState = buildFlowTopologyLayoutState(
	topology,
	"ownership",
	expandedStandaloneKinds,
	null,
);
const selectionTopology = buildBranchingTopology(1);
const rootSelectionNode = selectionTopology.nodes.find(
	(node) => node.kind === "CronJob",
);
const intermediateSelectionNode = selectionTopology.nodes.find(
	(node) => node.kind === "Job",
);
const sharedSelectionNode = selectionTopology.nodes.find(
	(node) =>
		node.kind === "Pod" &&
		selectionTopology.edges.filter((edge) => edge.target === node.id).length > 1,
);
const isolatedSelectionNode = selectionTopology.nodes.find(
	(node) => node.kind === "ConfigMap",
);
if (
	!rootSelectionNode ||
	!intermediateSelectionNode ||
	!sharedSelectionNode ||
	!isolatedSelectionNode
)
	throw new Error("Missing retained selection fixture nodes");
const selectionLayoutState = buildFlowTopologyLayoutState(
	selectionTopology,
	"networkFlow",
	new Set(),
	null,
);
const viewports = [
	{ width: 1280, height: 800 },
	{ width: 1440, height: 900 },
] as const;
const selectionCases = [
	{ label: "root", selectedNodeId: rootSelectionNode.id },
	{ label: "intermediate", selectedNodeId: intermediateSelectionNode.id },
	{ label: "shared", selectedNodeId: sharedSelectionNode.id },
	{ label: "isolated", selectedNodeId: isolatedSelectionNode.id },
	{ label: "missing", selectedNodeId: "missing-topology-node" },
];
const selectionLayoutPositions = new Map(
	selectionLayoutState.layout.nodes.map((node) => [node.id, node.position]),
);
const selectionCaseViews = selectionCases.map(({ selectedNodeId }) =>
	buildFlowTopologyViewFromLayoutState(selectionLayoutState, {
		mode: "networkFlow",
		selectedNodeId,
		showFullTopologyOnSelection: false,
		expandedStandaloneKinds,
		viewportSize: viewports[0],
	}),
);
const selectedView = buildFlowTopologyViewFromLayoutState(layoutState, {
	mode: "ownership",
	selectedNodeId: selectedIds[0] ?? null,
	showFullTopologyOnSelection: false,
	expandedStandaloneKinds,
	viewportSize: viewports[0],
});
const selectedFitPlan = buildFlowTopologyFitPlan(
	selectedView.graph.nodes,
	selectedView.graph.edges,
	selectedIds[0] ?? null,
	"ownership:1280x800",
	viewports[0],
);
const fullView = buildFlowTopologyViewFromLayoutState(layoutState, {
	mode: "ownership",
	selectedNodeId: null,
	showFullTopologyOnSelection: true,
	expandedStandaloneKinds,
	viewportSize: viewports[0],
});
const fullBounds = getFlowTopologyBounds(fullView.graph.nodes);
const fullFitPlan = buildFlowTopologyFitPlan(
	fullView.graph.nodes,
	fullView.graph.edges,
	null,
	"ownership:1280x800",
	viewports[0],
);
const resizedFitPlan = buildFlowTopologyFitPlan(
	fullView.graph.nodes,
	fullView.graph.edges,
	null,
	"ownership:1440x900",
	viewports[1],
);
const measuredNodes = fullView.graph.nodes.map((node) => ({
	...node,
	width: 220,
	height: 88,
	measured: { width: 220, height: 88 },
}));
const unmeasuredNodes = fullView.graph.nodes.map((node) => ({
	...node,
	width: undefined,
	height: undefined,
	measured: undefined,
}));
const missingParentNodes = fullView.graph.nodes.map((node, index) =>
	index === 0 ? { ...node, parentId: "missing-parent" } : node,
);
const measuredBounds = getFlowTopologyBounds(measuredNodes);
const unmeasuredBounds = getFlowTopologyBounds(unmeasuredNodes);
const missingParentBounds = getFlowTopologyBounds(missingParentNodes);
const measuredFitPlan = buildFlowTopologyFitPlan(
	measuredNodes,
	fullView.graph.edges,
	null,
	"ownership:measured",
	viewports[0],
);
const unmeasuredFitPlan = buildFlowTopologyFitPlan(
	unmeasuredNodes,
	fullView.graph.edges,
	null,
	"ownership:unmeasured",
	viewports[0],
);
const missingParentFitPlan = buildFlowTopologyFitPlan(
	missingParentNodes,
	fullView.graph.edges,
	null,
	"ownership:missing-parent",
	viewports[0],
);

assert.equal(selectedIds.length, 50);
assert.equal(fullView.graph.nodes.length, layoutState.layout.nodes.length);
assert(selectedView.graph.nodes.some((node) => node.data.selected));
assert(selectedFitPlan?.focused);
assert(fullFitPlan && !fullFitPlan.focused);
assert(fullView.graph.nodes.every((node) => !node.data.selected));
assert(resizedFitPlan && resizedFitPlan.key !== fullFitPlan.key);
assert(selectedFitPlan.key !== fullFitPlan.key);
assert(fullBounds && fullBounds.width > 0 && fullBounds.height > 0);
assert(measuredBounds && measuredBounds.width > 0 && measuredBounds.height > 0);
assert(unmeasuredBounds && unmeasuredBounds.width > 0 && unmeasuredBounds.height > 0);
assert(missingParentBounds && missingParentBounds.width > 0 && missingParentBounds.height > 0);
assert(measuredFitPlan && Number.isFinite(measuredFitPlan.viewport.zoom));
assert(unmeasuredFitPlan && Number.isFinite(unmeasuredFitPlan.viewport.zoom));
assert(missingParentFitPlan && Number.isFinite(missingParentFitPlan.viewport.zoom));
assert.equal(buildFlowTopologyFitPlan([], [], null, "empty", viewports[0]), null);
assert.equal(
	buildFlowTopologyFitPlan(
		fullView.graph.nodes,
		fullView.graph.edges,
		null,
		"zero-size",
		{ width: 0, height: 0 },
	),
	null,
);
assert(
	selectionCaseViews.slice(0, 4).every((view, index) =>
		view.graph.nodes.some(
			(node) => node.data.selected && node.id === selectionCases[index]?.selectedNodeId,
		),
	),
);
assert.equal(
	selectionCaseViews[4]?.graph.nodes.some((node) => node.data.selected),
	false,
);
assert(
	selectionCaseViews.every((view) =>
		view.graph.nodes.every((node) => {
			const position = selectionLayoutPositions.get(node.id);
			return !position || (node.position.x === position.x && node.position.y === position.y);
		}),
	),
);
assert(
	selectedView.graph.nodes.every(
		(node) =>
			Number.isFinite(node.position.x) && Number.isFinite(node.position.y),
	),
);

describe("F07 graph layout and F09 retained topology selection (500 apps / 1k nodes)", () => {
	bench("F07 layout construction per selection (50 full rebuilds)", () => {
		for (const selectedId of selectedIds)
			buildFlowTopology(topology, selectedId);
	});

	bench("F07 ownership layout (build once)", () => {
		buildFlowTopologyLayout(topology, null);
	});

	bench("F09 indexed selection (50 updates)", () => {
		for (const selectedId of selectedIds) {
			applyFlowTopologySelectionWithIndex(layout, selectionIndex, selectedId);
		}
	});

	bench("F09 selection + resize (rebuild view, 50 updates)", () => {
		for (const [index, selectedNodeId] of selectedIds.entries()) {
			buildFlowTopologyView(topology, {
				mode: "ownership",
				selectedNodeId,
				showFullTopologyOnSelection: index % 2 === 0,
				expandedStandaloneKinds,
				viewportSize: viewports[index % viewports.length],
			});
		}
	});

	bench("F09 selection + resize (retained layout, 50 updates)", () => {
		for (const [index, selectedNodeId] of selectedIds.entries()) {
			buildFlowTopologyViewFromLayoutState(layoutState, {
				mode: "ownership",
				selectedNodeId,
				showFullTopologyOnSelection: index % 2 === 0,
				expandedStandaloneKinds,
				viewportSize: viewports[index % viewports.length],
			});
		}
		for (const { selectedNodeId } of selectionCases) {
			buildFlowTopologyViewFromLayoutState(selectionLayoutState, {
				mode: "networkFlow",
				selectedNodeId,
				showFullTopologyOnSelection: false,
				expandedStandaloneKinds,
				viewportSize: viewports[0],
			});
		}
	});

	bench("F10 full and focused fit plans across viewport geometry", () => {
		buildFlowTopologyFitPlan(
			selectedView.graph.nodes,
			selectedView.graph.edges,
			selectedIds[0] ?? null,
			"ownership:1280x800",
			viewports[0],
		);
		buildFlowTopologyFitPlan(
			fullView.graph.nodes,
			fullView.graph.edges,
			null,
			"ownership:1280x800",
			viewports[0],
		);
		buildFlowTopologyFitPlan(
			fullView.graph.nodes,
			fullView.graph.edges,
			null,
			"ownership:1440x900",
			viewports[1],
		);
		buildFlowTopologyFitPlan(measuredNodes, fullView.graph.edges, null, "measured", viewports[0]);
		buildFlowTopologyFitPlan(unmeasuredNodes, fullView.graph.edges, null, "unmeasured", viewports[0]);
		buildFlowTopologyFitPlan(missingParentNodes, fullView.graph.edges, null, "missing-parent", viewports[0]);
		buildFlowTopologyFitPlan([], [], null, "empty", viewports[0]);
	});
});
