import assert from "node:assert/strict";
import { bench, describe } from "vitest";
import { buildFlowTopologyLayoutState } from "@/features/resources/topology-view";
import {
	addStandaloneTopologyResources,
	buildBranchingTopology,
} from "./topology-benchmark-fixtures";

const topology = buildBranchingTopology(250);
const standaloneTopology = addStandaloneTopologyResources(topology);
const collapsedKinds = new Set<string>();
const expandedKinds = new Set(["ConfigMap", "Node", "Widget"]);
const expandedStandaloneNode = standaloneTopology.nodes.find(
	(node) => node.kind === "Widget",
);
const firstTopologyNode = topology.nodes[0];
if (!firstTopologyNode) throw new Error("Missing topology root fixture node");
const firstPod = topology.nodes.find((node) => node.kind === "Pod");
if (!firstPod) throw new Error("Missing topology Pod fixture node");
const malformedTopology = {
	...topology,
	nodes: [...topology.nodes, firstTopologyNode],
	edges: [
		...topology.edges,
		{
			id: "cycle-to-root",
			source: firstPod.id,
			target: firstTopologyNode.id,
			relation: "owns" as const,
		},
		{
			id: "dangling",
			source: "missing-node",
			target: firstTopologyNode.id,
			relation: "owns" as const,
		},
	],
};
const expandedLayout = buildFlowTopologyLayoutState(
	standaloneTopology,
	"ownership",
	expandedKinds,
	expandedStandaloneNode?.id ?? null,
);
const selectedStandaloneLayout = buildFlowTopologyLayoutState(
	standaloneTopology,
	"ownership",
	collapsedKinds,
	expandedStandaloneNode?.id ?? null,
);
const networkLayout = buildFlowTopologyLayoutState(
	standaloneTopology,
	"networkFlow",
	collapsedKinds,
	null,
);
const malformedLayout = buildFlowTopologyLayoutState(
	malformedTopology,
	"ownership",
	collapsedKinds,
	null,
);
const repeatedMalformedLayout = buildFlowTopologyLayoutState(
	malformedTopology,
	"ownership",
	collapsedKinds,
	null,
);
const collapsedGroupSummary = buildFlowTopologyLayoutState(
	standaloneTopology,
	"ownership",
	collapsedKinds,
	null,
).layout.nodes
	.filter((node) => node.type === "standaloneKindGroup")
	.map((node) => ({
		id: node.id,
		kind: node.data.kind,
		count: node.data.count,
		expanded: node.data.expanded,
		position: node.position,
	}));
const expandedGroupSummary = expandedLayout.layout.nodes
	.filter((node) => node.type === "standaloneKindGroup")
	.map((node) => ({
		id: node.id,
		kind: node.data.kind,
		count: node.data.count,
		expanded: node.data.expanded,
		position: node.position,
	}));
const repeatedExpandedLayout = buildFlowTopologyLayoutState(
	standaloneTopology,
	"ownership",
	expandedKinds,
	expandedStandaloneNode?.id ?? null,
);
const repeatedExpandedGroupSummary = repeatedExpandedLayout.layout.nodes
	.filter((node) => node.type === "standaloneKindGroup")
	.map((node) => ({
		id: node.id,
		kind: node.data.kind,
		count: node.data.count,
		expanded: node.data.expanded,
		position: node.position,
	}));
const repeatedCollapsedGroupSummary = buildFlowTopologyLayoutState(
	standaloneTopology,
	"ownership",
	collapsedKinds,
	null,
).layout.nodes
	.filter((node) => node.type === "standaloneKindGroup")
	.map((node) => ({
		id: node.id,
		kind: node.data.kind,
		count: node.data.count,
		expanded: node.data.expanded,
		position: node.position,
	}));

assert.equal(topology.nodes.length, 2_100);
assert.equal(topology.edges.length, 1_750);
assert.equal(standaloneTopology.nodes.length, topology.nodes.length + 2);
assert.equal(
	new Set(topology.nodes.map((node) => node.summary.createdAt)).size,
	1,
);
assert.deepEqual(collapsedGroupSummary, repeatedCollapsedGroupSummary);
assert.deepEqual(expandedGroupSummary, repeatedExpandedGroupSummary);
assert(
	collapsedGroupSummary.every((group) => group.expanded === false),
);
assert.equal(
	collapsedGroupSummary.find((group) => group.kind === "ConfigMap")?.count,
	100,
);
assert.equal(collapsedGroupSummary.find((group) => group.kind === "Node")?.count, 1);
assert.equal(collapsedGroupSummary.find((group) => group.kind === "Widget")?.count, 1);
assert(
	expandedGroupSummary
		.filter((group) => ["ConfigMap", "Node", "Widget"].includes(group.kind ?? ""))
		.every((group) => group.expanded),
);
assert(
	expandedLayout.layout.nodes.some(
		(node) => node.type === "standaloneKindGroup",
	),
);
assert.equal(
	selectedStandaloneLayout.layout.nodes.find(
		(node) => node.type === "standaloneKindGroup" && node.data.kind === "Widget",
	)?.data.expanded,
	true,
);
assert.equal(
	selectedStandaloneLayout.layout.nodes.find(
		(node) => node.type === "standaloneKindGroup" && node.data.kind === "Node",
	)?.data.expanded,
	false,
);
assert(networkLayout.layout.nodes.every((node) => node.data.showPortHints));
assert.equal(
	networkLayout.layout.nodes.some((node) => node.type === "standaloneKindGroup"),
	false,
);
assert(
	malformedLayout.layout.nodes.every(
		(node) =>
			Number.isFinite(node.position.x) && Number.isFinite(node.position.y),
	),
);
assert(
	malformedLayout.layout.edges.every(
		(edge) =>
			malformedLayout.layout.nodes.some((node) => node.id === edge.source) &&
			malformedLayout.layout.nodes.some((node) => node.id === edge.target),
	),
);
assert.equal(
	new Set(malformedLayout.layout.nodes.map((node) => node.id)).size,
	malformedLayout.layout.nodes.length,
);
assert(
	malformedLayout.layout.nodes.every(
		(node, index) =>
			JSON.stringify({ id: node.id, parentId: node.parentId, position: node.position }) ===
				JSON.stringify({
					id: repeatedMalformedLayout.layout.nodes[index]?.id,
					parentId: repeatedMalformedLayout.layout.nodes[index]?.parentId,
					position: repeatedMalformedLayout.layout.nodes[index]?.position,
				}),
	),
);
assert.equal(
	malformedLayout.layout.edges.some((edge) => edge.id === "dangling"),
	false,
);

describe("F07 branching layout with folded F08 standalone and network branches", () => {
	bench("F07 production ownership layout state for branching graph", () => {
		buildFlowTopologyLayoutState(topology, "ownership", collapsedKinds, null);
	});

	bench("F08 collapsed standalone groups", () => {
		buildFlowTopologyLayoutState(
			standaloneTopology,
			"ownership",
			collapsedKinds,
			null,
		);
	});

	bench("F08 expanded standalone group and selected standalone node", () => {
		buildFlowTopologyLayoutState(
			standaloneTopology,
			"ownership",
			expandedKinds,
			expandedStandaloneNode?.id ?? null,
		);
		buildFlowTopologyLayoutState(
			standaloneTopology,
			"ownership",
			collapsedKinds,
			expandedStandaloneNode?.id ?? null,
		);
	});

	bench("F08 network-flow layout", () => {
		buildFlowTopologyLayoutState(
			standaloneTopology,
			"networkFlow",
			collapsedKinds,
			null,
		);
	});

	bench("F07 duplicate node and dangling edge normalization", () => {
		buildFlowTopologyLayoutState(
			malformedTopology,
			"ownership",
			collapsedKinds,
			null,
		);
	});
});
