// The edge layer replaces Svelte Flow's edge components on large maps, so it is wrong whenever it
// disagrees with Svelte Flow. Failure modes, checked against @xyflow/system's own functions:
// - path geometry drifts (handle anchor, smoothstep defaults, ignored pathOptions);
// - a selected node's edges are not lifted to its z, so they draw under other nodes;
// - culling hides an edge Svelte Flow would draw, or the reverse;
// - edges touching nested nodes, whose position and z come from the parent, are drawn by the layer;
// - declared handles disagree with the measured ones (checked by comparing against CSS bounds), or
//   are missing, so Svelte Flow mounts every off-screen node once to measure it.
import {
	adoptUserNodes,
	ConnectionMode,
	getEdgePosition,
	getElevatedEdgeZIndex,
	getNodesInside,
	getSmoothStepPath,
	type InternalNodeBase,
	isEdgeVisible,
} from "@xyflow/system";
import type { ResourceTopology, TopologyNode } from "@/lib/types";
import { buildFlowTopologyView } from "./topology";
import { buildFlowTopologyEdgeLayer, topologyLayerEdgeVisible } from "./topology-edge-layer";
import type { FlowTopologyNode } from "./topology-types";

declare function describe(name: string, fn: () => void): void;
declare function test(name: string, fn: () => void): void;
declare function expect<T>(actual: T): {
	toBe<Expected>(expected: Expected): void;
	toEqual<Expected>(expected: Expected): void;
};

function node(kind: string, name: string): TopologyNode {
	return {
		id: `${kind}:${name}`,
		kind,
		name,
		namespace: "payments",
		status: "healthy",
		health: "healthy",
		selectable: true,
		summary: { kind, cluster: "test", name, namespace: "payments", age: "1m", apiVersion: "v1", health: "healthy" },
	};
}

// A Deployment's ReplicaSet fanning out to Pods: the shape that mounted every Pod before the layer.
function fanOut(pods: number): ResourceTopology {
	const nodes = [node("Deployment", "api"), node("ReplicaSet", "api-1"), node("Secret", "unused")];
	const edges: ResourceTopology["edges"] = [
		{ id: "deploy-rs", source: "Deployment:api", target: "ReplicaSet:api-1", relation: "owns" },
	];
	for (let index = 0; index < pods; index++) {
		nodes.push(node("Pod", `api-1-${index}`));
		edges.push({ id: `rs-pod-${index}`, source: "ReplicaSet:api-1", target: `Pod:api-1-${index}`, relation: "owns" });
	}
	return { nodes, edges, warnings: [] };
}

function graph(selectedNodeId: string | null) {
	return buildFlowTopologyView(fanOut(150), {
		mode: "ownership",
		selectedNodeId,
		showFullTopologyOnSelection: true,
		expandedStandaloneKinds: new Set(),
	}).graph;
}

function present<T>(value: T | null | undefined, what: string): T {
	if (value === null || value === undefined) throw new Error(`missing ${what}`);
	return value;
}

// Internal nodes as Svelte Flow holds them, with handle bounds as it measures them from its
// stylesheet: 6px handles centered on the left and right borders.
function internalNodes(nodes: FlowTopologyNode[]) {
	const lookup = new Map<string, InternalNodeBase<FlowTopologyNode>>();
	adoptUserNodes(nodes, lookup, new Map(), { elevateNodesOnSelect: true });
	for (const internal of lookup.values()) {
		const width = internal.measured.width ?? 0;
		const height = internal.measured.height ?? 0;
		const handle = { id: null, width: 6, height: 6, nodeId: internal.id, y: height / 2 - 3 };
		const { sourcePosition: source, targetPosition: target } = internal;
		// Standalone kind groups have no handles and no edges.
		if (!source || !target) continue;
		internal.internals.handleBounds = {
			source: [{ ...handle, type: "source", position: source, x: width - 3 }],
			target: [{ ...handle, type: "target", position: target, x: -3 }],
		};
	}
	return lookup;
}

const viewports = [
	{ viewport: { x: 0, y: 0, zoom: 1 }, width: 800, height: 450 },
	{ viewport: { x: -900, y: -2400, zoom: 0.5 }, width: 640, height: 480 },
	{ viewport: { x: 120, y: -9000, zoom: 1.4 }, width: 1280, height: 720 },
];

describe("topology edge layer", () => {
	for (const selected of [null, "ReplicaSet:api-1", "Pod:api-1-7"]) {
		test(`matches Svelte Flow geometry, z, and culling with selection ${selected}`, () => {
			const flow = graph(selected);
			const lookup = internalNodes(flow.nodes);
			const layer = buildFlowTopologyEdgeLayer(flow.nodes, flow.edges);
			// The view's left border touches the ReplicaSet's right border: zero overlap is not visible.
			const replicaSet = present(lookup.get("ReplicaSet:api-1"), "ReplicaSet");
			const touchingRight = replicaSet.internals.positionAbsolute.x + (replicaSet.measured.width ?? 0);
			const views = [
				...viewports,
				{
					viewport: { x: -touchingRight, y: -replicaSet.internals.positionAbsolute.y, zoom: 1 },
					width: 800,
					height: 450,
				},
			];
			expect(layer.native.length).toBe(0);
			expect(layer.layered.length).toBe(flow.edges.length);
			for (const [index, edge] of flow.edges.entries()) {
				const layered = layer.layered[index];
				const sourceNode = present(lookup.get(edge.source), edge.source);
				const targetNode = present(lookup.get(edge.target), edge.target);
				const position = present(
					getEdgePosition({
						id: edge.id,
						sourceNode,
						targetNode,
						sourceHandle: null,
						targetHandle: null,
						connectionMode: ConnectionMode.Strict,
					}),
					`${edge.id} position`,
				);
				expect(layered.id).toBe(edge.id);
				expect(layered.path).toBe(getSmoothStepPath(position)[0]);
				expect(layered.style).toBe(edge.style);
				expect(layered.markerEnd).toEqual(edge.markerEnd);
				expect(layered.z).toBe(
					getElevatedEdgeZIndex({ sourceNode, targetNode, zIndex: edge.zIndex ?? 0, elevateOnSelect: true }),
				);
				for (const view of views) {
					const { x, y, zoom } = view.viewport;
					expect(topologyLayerEdgeVisible(layered, view)).toBe(
						isEdgeVisible({ sourceNode, targetNode, width: view.width, height: view.height, transform: [x, y, zoom] }),
					);
				}
			}
		});
	}

	test("lets Svelte Flow cull resource nodes before it measures them", () => {
		const flow = graph(null);
		const lookup = new Map<string, InternalNodeBase<FlowTopologyNode>>();
		adoptUserNodes(flow.nodes, lookup, new Map(), { elevateNodesOnSelect: true });
		for (const view of viewports) {
			const { x, y, zoom } = view.viewport;
			const rendered = getNodesInside(lookup, { x: 0, y: 0, width: view.width, height: view.height }, [x, y, zoom], true);
			const left = -x / zoom;
			const top = -y / zoom;
			const resources = rendered.filter((entry) => entry.type === "ownershipResource");
			expect(resources.length < flow.nodes.length / 2).toBe(true);
			for (const entry of resources) {
				const { x: nodeX, y: nodeY } = entry.internals.positionAbsolute;
				const overlaps =
					nodeX < left + view.width / zoom &&
					nodeX + (entry.measured.width ?? 0) > left &&
					nodeY < top + view.height / zoom &&
					nodeY + (entry.measured.height ?? 0) > top;
				expect(`${entry.id} ${overlaps}`).toBe(`${entry.id} true`);
			}
		}
	});

	test("leaves edges of nested nodes to Svelte Flow", () => {
		const flow = graph(null);
		const nodes = flow.nodes.map((entry) =>
			entry.id === "Pod:api-1-3" ? { ...entry, parentId: "Deployment:api" } : entry,
		);
		const layer = buildFlowTopologyEdgeLayer(nodes, flow.edges);
		expect(layer.native.map((edge) => edge.id)).toEqual(["rs-pod-3"]);
		expect(layer.layered.some((edge) => edge.id === "rs-pod-3")).toBe(false);
	});
});
