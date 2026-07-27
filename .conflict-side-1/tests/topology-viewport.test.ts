import { describe, expect, test } from "bun:test";
import {
	buildFlowTopology,
	type FlowTopologyNode,
	getFlowTopologyBounds,
	getTopologyTranslateExtent,
} from "../src/features/resources/topology";
import type { ResourceSummary, ResourceTopology, TopologyNode } from "../src/lib/types";

function graphNode(
	id: string,
	x: number,
	y: number,
	width: number,
	height: number,
	parentId?: string,
): FlowTopologyNode {
	return {
		id,
		type: "ownershipResource",
		position: { x, y },
		parentId,
		width,
		height,
		data: {
			resource: null,
			label: id,
			selected: false,
			connected: false,
		},
	};
}

function topologyNode(name: string): TopologyNode {
	const summary: ResourceSummary = {
		cluster: "kind-dev",
		apiVersion: "v1",
		kind: "Secret",
		name,
		namespace: "default",
		age: "1m",
	};
	return {
		id: `Secret:${name}`,
		kind: "Secret",
		name,
		namespace: "default",
		health: "healthy",
		selectable: true,
		summary,
	};
}

describe("topology viewport invariants", () => {
	test("selected child bounds include parent offsets", () => {
		const parent = graphNode("group", 100, 50, 300, 200);
		const child = graphNode("child", 20, 30, 50, 40, parent.id);

		expect(getFlowTopologyBounds([parent, child], [child])).toEqual({
			left: 120,
			top: 80,
			right: 170,
			bottom: 120,
			width: 50,
			height: 40,
		});
	});

	test("expanded standalone groups increase graph bounds", () => {
		const topology: ResourceTopology = {
			nodes: Array.from({ length: 5 }, (_, index) => topologyNode(`secret-${index}`)),
			edges: [],
			warnings: [],
		};
		const collapsed = buildFlowTopology(topology, null);
		const expanded = buildFlowTopology(topology, null, "ownership", {
			expandedStandaloneKinds: new Set(["Secret"]),
		});

		expect(getFlowTopologyBounds(expanded.nodes)?.bottom).toBeGreaterThan(
			getFlowTopologyBounds(collapsed.nodes)?.bottom ?? 0,
		);
	});

	test("empty topology returns zero translate extent", () => {
		expect(getTopologyTranslateExtent([], { width: 1200, height: 640 })).toEqual([
			[0, 0],
			[0, 0],
		]);
	});
});
