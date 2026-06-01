import { describe, expect, test } from "bun:test";
import type { ResourceSummary, ResourceTopology, TopologyNode } from "../src/lib/types";
import {
	buildReactFlowTopology,
	resourceTopologyNodeId,
	type OwnershipGraphNode,
} from "../src/features/resources/topology";
import {
	getOwnershipGraphBounds,
	getOwnershipGraphBoundsForNodeIds,
	getOwnershipMapTranslateExtent,
	ownershipMapBoundaryPadding,
} from "../src/features/resources/topology-viewport";

function summary(overrides: Partial<ResourceSummary>): ResourceSummary {
	return {
		cluster: "kind-dev",
		kind: "Pod",
		name: "api-0",
		namespace: "default",
		age: "1m",
		status: "Running",
		ready: "true",
		...overrides,
	};
}

function topologyNode(kind: string, name: string): TopologyNode {
	const apiVersion = kind === "Deployment" ? "apps/v1" : "v1";
	return {
		id: resourceTopologyNodeId("kind-dev", apiVersion, kind, "default", name),
		kind,
		name,
		namespace: "default",
		status: null,
		health: "unknown",
		selectable: true,
		summary: summary({ apiVersion, kind, name }),
	};
}

function graphNode(id: string, x: number, y: number): OwnershipGraphNode {
	const node = topologyNode("Pod", id);
	return {
		id,
		type: "ownershipResource",
		position: { x, y },
		data: {
			node,
			resource: null,
			selected: false,
			connected: false,
			dimmed: false,
			standalone: false,
		},
		style: { width: 190 },
	} as OwnershipGraphNode;
}

describe("ownership map viewport helpers", () => {
	test("collapsed ownerless-only graph produces a finite translate extent", () => {
		const topology: ResourceTopology = {
			nodes: [
				topologyNode("ConfigMap", "kube-root-ca.crt"),
				topologyNode("Secret", "argocd-redis"),
			],
			edges: [],
			warnings: [],
		};
		const graph = buildReactFlowTopology(topology, null);
		const extent = getOwnershipMapTranslateExtent(graph.nodes, {
			width: 1200,
			height: 640,
		});

		for (const value of extent.flat()) {
			expect(Number.isFinite(value)).toBe(true);
		}
		expect(extent[0][0]).toBeLessThan(extent[1][0]);
		expect(extent[0][1]).toBeLessThan(extent[1][1]);
	});

	test("expanded ownerless groups increase the computed graph bottom", () => {
		const topology: ResourceTopology = {
			nodes: Array.from({ length: 5 }, (_, index) =>
				topologyNode("Secret", `secret-${index}`),
			),
			edges: [],
			warnings: [],
		};
		const collapsedGraph = buildReactFlowTopology(topology, null);
		const expandedGraph = buildReactFlowTopology(topology, null, {
			expandedStandaloneKinds: new Set(["Secret"]),
		});
		const collapsedBounds = getOwnershipGraphBounds(collapsedGraph.nodes);
		const expandedBounds = getOwnershipGraphBounds(expandedGraph.nodes);

		expect(expandedBounds?.bottom).toBeGreaterThan(collapsedBounds?.bottom ?? 0);
	});

	test("child nodes inside parent groups are measured at absolute positions", () => {
		const child = topologyNode("ConfigMap", "config");
		const nodes = [
			{
				id: "parent",
				type: "standaloneKindGroup",
				position: { x: 100, y: 50 },
				data: {
					kind: "ConfigMap",
					count: 1,
					dimmed: false,
					expanded: true,
				},
				style: { width: 120, height: 60 },
			},
			{
				id: "child",
				type: "ownershipResource",
				parentId: "parent",
				position: { x: 220, y: 110 },
				data: {
					node: child,
					resource: null,
					selected: false,
					connected: false,
					dimmed: false,
					standalone: true,
				},
				style: { width: 260 },
			},
		] as OwnershipGraphNode[];
		const bounds = getOwnershipGraphBounds(nodes);

		expect(bounds?.right).toBe(580);
		expect(bounds?.bottom).toBe(238);
	});

	test("selected child bounds keep parent group offsets", () => {
		const child = topologyNode("ConfigMap", "config");
		const nodes = [
			{
				id: "parent",
				type: "standaloneKindGroup",
				position: { x: 100, y: 50 },
				data: {
					kind: "ConfigMap",
					count: 1,
					dimmed: false,
					expanded: true,
				},
				style: { width: 120, height: 60 },
			},
			{
				id: "child",
				type: "ownershipResource",
				parentId: "parent",
				position: { x: 220, y: 110 },
				data: {
					node: child,
					resource: null,
					selected: true,
					connected: false,
					dimmed: false,
					standalone: true,
				},
				style: { width: 260 },
			},
		] as OwnershipGraphNode[];
		const bounds = getOwnershipGraphBoundsForNodeIds(
			nodes,
			new Set(["child"]),
		);

		expect(bounds?.left).toBe(320);
		expect(bounds?.top).toBe(160);
		expect(bounds?.right).toBe(580);
		expect(bounds?.bottom).toBe(238);
	});

	test("wide viewports produce larger horizontal padding than vertical padding", () => {
		const padding = ownershipMapBoundaryPadding({ width: 1400, height: 400 });

		expect(padding.x).toBeGreaterThan(padding.y);
		expect(padding.x).toBe(588);
		expect(padding.y).toBe(180);
	});

	test("large graphs use viewport-based padding instead of item-count padding", () => {
		const viewportSize = { width: 1000, height: 600 };
		const smallGraph = [graphNode("pod-a", 96, 96)];
		const largeGraph = Array.from({ length: 80 }, (_, index) =>
			graphNode(`pod-${index}`, 96 + index * 14, 96 + index * 9),
		);
		const smallBounds = getOwnershipGraphBounds(smallGraph);
		const largeBounds = getOwnershipGraphBounds(largeGraph);
		const smallExtent = getOwnershipMapTranslateExtent(smallGraph, viewportSize);
		const largeExtent = getOwnershipMapTranslateExtent(largeGraph, viewportSize);
		const smallPaddingX = (smallBounds?.left ?? 0) - smallExtent[0][0];
		const largePaddingX = (largeBounds?.left ?? 0) - largeExtent[0][0];
		const smallPaddingY = (smallBounds?.top ?? 0) - smallExtent[0][1];
		const largePaddingY = (largeBounds?.top ?? 0) - largeExtent[0][1];

		expect(smallPaddingX).toBe(420);
		expect(largePaddingX).toBe(smallPaddingX);
		expect(smallPaddingY).toBe(252);
		expect(largePaddingY).toBe(smallPaddingY);
	});

	test("returns a safe zero extent before viewport dimensions are measured", () => {
		const extent = getOwnershipMapTranslateExtent([graphNode("pod-a", 96, 96)], {
			width: 0,
			height: 600,
		});

		expect(extent).toEqual([
			[0, 0],
			[0, 0],
		]);
	});
});
