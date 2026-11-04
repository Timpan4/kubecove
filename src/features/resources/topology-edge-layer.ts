import { type EdgeMarker, getSmoothStepPath, Position } from "@xyflow/system";
import type { FlowTopologyEdge, FlowTopologyNode } from "./topology-types";

// Large maps draw edges in one SVG layer instead of one Svelte Flow edge component per edge.
// With onlyRenderVisibleElements, Svelte Flow mounts both endpoints of every edge that crosses the
// viewport, so a ReplicaSet fanning out to 10k Pods mounted 10k nodes and 10k edge components.
// The layer reproduces Svelte Flow's smoothstep geometry, z-order, markers, and edge labels.

// @xyflow/svelte's stylesheet centers 6px border-box handles on the node border
// (`translate(±50%, -50%)`), so an edge starts and ends 3px outside the node box.
const HANDLE_OVERHANG = 3;
// @xyflow/system raises a selected node by 1000 and lifts its edges to the node's z.
const SELECTED_NODE_Z = 1000;

export type TopologyLayerEdge = {
	id: string;
	source: string;
	target: string;
	path: string;
	/** Union of both endpoint boxes, as Svelte Flow's isEdgeVisible uses. */
	box: { x: number; y: number; x2: number; y2: number };
	z: number;
	style: string | undefined;
	markerEnd: EdgeMarker | undefined;
};

export type FlowTopologyEdgeLayer = {
	/** Edges Svelte Flow still renders: those touching nested (parented) nodes, whose z and position depend on the parent. */
	native: FlowTopologyEdge[];
	layered: TopologyLayerEdge[];
};

export type FlowViewportBox = {
	viewport: { x: number; y: number; zoom: number };
	width: number;
	height: number;
};

function handlePoint(node: FlowTopologyNode, position: Position, width: number, height: number) {
	const { x, y } = node.position;
	switch (position) {
		case Position.Left:
			return { x: x - HANDLE_OVERHANG, y: y + height / 2 };
		case Position.Right:
			return { x: x + width + HANDLE_OVERHANG, y: y + height / 2 };
		case Position.Top:
			return { x: x + width / 2, y: y - HANDLE_OVERHANG };
		case Position.Bottom:
			return { x: x + width / 2, y: y + height + HANDLE_OVERHANG };
	}
}

function nodeZ(node: FlowTopologyNode): number {
	return node.selected ? (node.zIndex ?? 0) + SELECTED_NODE_Z : 0;
}

export function buildFlowTopologyEdgeLayer(
	nodes: readonly FlowTopologyNode[],
	edges: readonly FlowTopologyEdge[],
): FlowTopologyEdgeLayer {
	const nodesById = new Map<string, FlowTopologyNode>();
	for (const node of nodes) nodesById.set(node.id, node);
	const native: FlowTopologyEdge[] = [];
	const layered: TopologyLayerEdge[] = [];
	for (const edge of edges) {
		const source = nodesById.get(edge.source);
		const target = nodesById.get(edge.target);
		if (!source || !target || source.hidden || target.hidden || edge.hidden) continue;
		if (
			source.parentId ||
			target.parentId ||
			source.width === undefined ||
			source.height === undefined ||
			target.width === undefined ||
			target.height === undefined
		) {
			native.push(edge);
			continue;
		}
		const sourcePosition = source.sourcePosition ?? Position.Bottom;
		const targetPosition = target.targetPosition ?? Position.Top;
		const start = handlePoint(source, sourcePosition, source.width, source.height);
		const end = handlePoint(target, targetPosition, target.width, target.height);
		// Svelte Flow's built-in smoothstep edge passes only the endpoints, so pathOptions do not apply.
		const [path] = getSmoothStepPath({
			sourceX: start.x,
			sourceY: start.y,
			sourcePosition,
			targetX: end.x,
			targetY: end.y,
			targetPosition,
		});
		layered.push({
			id: edge.id,
			source: edge.source,
			target: edge.target,
			path,
			box: {
				x: Math.min(source.position.x, target.position.x),
				y: Math.min(source.position.y, target.position.y),
				x2: Math.max(source.position.x + source.width, target.position.x + target.width),
				y2: Math.max(source.position.y + source.height, target.position.y + target.height),
			},
			z: (edge.zIndex ?? 0) + Math.max(nodeZ(source), nodeZ(target)),
			style: edge.style,
			markerEnd: edge.markerEnd,
		});
	}
	return { native, layered };
}

/** Svelte Flow's isEdgeVisible: the endpoint union box overlaps the viewport with positive area. */
export function topologyLayerEdgeVisible(edge: TopologyLayerEdge, view: FlowViewportBox): boolean {
	const { x, y, zoom } = view.viewport;
	const left = -x / zoom;
	const top = -y / zoom;
	const x2 = edge.box.x2 === edge.box.x ? edge.box.x2 + 1 : edge.box.x2;
	const y2 = edge.box.y2 === edge.box.y ? edge.box.y2 + 1 : edge.box.y2;
	return (
		Math.min(x2, left + view.width / zoom) > Math.max(edge.box.x, left) &&
		Math.min(y2, top + view.height / zoom) > Math.max(edge.box.y, top)
	);
}
