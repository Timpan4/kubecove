import { type EdgeMarker, getSmoothStepPath, type NodeHandle, Position } from "@xyflow/system";
import { resourceLifecycle } from "./resource-lifecycle";
import { type FlowTopologyEdge, type FlowTopologyNode, INACTIVE_EDGE_CLASS } from "./topology-types";

// Large maps draw edges in one SVG layer instead of one Svelte Flow edge component per edge.
// With onlyRenderVisibleElements, Svelte Flow mounts both endpoints of every edge that crosses the
// viewport, so a ReplicaSet fanning out to 10k Pods mounted 10k nodes and 10k edge components.
// The layer reproduces Svelte Flow's smoothstep geometry, z-order, markers, and edge labels.

// @xyflow/svelte's stylesheet centers 6px border-box handles on the node border (`translate(±50%, -50%)`).
const HANDLE_SIZE = 6;
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

/**
 * A resource node's handles as Svelte Flow would measure them: target on the left border, source
 * on the right. Declaring them lets Svelte Flow place edges and cull nodes before it measures any;
 * without them, onlyRenderVisibleElements mounts every node once just to measure it.
 */
export function ownershipNodeHandles(width: number, height: number): NodeHandle[] {
	const y = height / 2 - HANDLE_SIZE / 2;
	const size = { width: HANDLE_SIZE, height: HANDLE_SIZE };
	return [
		{ type: "target", position: Position.Left, x: -HANDLE_SIZE / 2, y, ...size },
		{ type: "source", position: Position.Right, x: width - HANDLE_SIZE / 2, y, ...size },
	];
}

/** @xyflow/system's getHandlePosition for a declared handle. */
function handlePoint(node: FlowTopologyNode, handle: NodeHandle) {
	const x = node.position.x + handle.x;
	const y = node.position.y + handle.y;
	const width = handle.width ?? 1;
	const height = handle.height ?? 1;
	switch (handle.position) {
		case Position.Left:
			return { x, y: y + height / 2 };
		case Position.Right:
			return { x: x + width, y: y + height / 2 };
		case Position.Top:
			return { x: x + width / 2, y };
		case Position.Bottom:
			return { x: x + width / 2, y: y + height };
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
		const geometry = edgeGeometry(source, target);
		if (!geometry) {
			native.push(edge);
			continue;
		}
		layered.push({
			id: edge.id,
			source: edge.source,
			target: edge.target,
			path: geometry.path,
			box: geometry.box,
			z: (edge.zIndex ?? 0) + Math.max(nodeZ(source), nodeZ(target)),
			style: edge.style,
			markerEnd: edge.markerEnd,
		});
	}
	return { native, layered };
}

/**
 * The smoothstep path Svelte Flow draws between two nodes, and the union of their boxes. Null for
 * nested (parented) nodes, whose position depends on the parent, and for nodes without declared
 * handles or size.
 */
function edgeGeometry(
	source: FlowTopologyNode,
	target: FlowTopologyNode,
): Pick<TopologyLayerEdge, "path" | "box"> | null {
	const sourceHandle = source.handles?.find((handle) => handle.type === "source");
	const targetHandle = target.handles?.find((handle) => handle.type === "target");
	if (
		source.parentId ||
		target.parentId ||
		!sourceHandle ||
		!targetHandle ||
		source.width === undefined ||
		source.height === undefined ||
		target.width === undefined ||
		target.height === undefined
	) {
		return null;
	}
	const start = handlePoint(source, sourceHandle);
	const end = handlePoint(target, targetHandle);
	// Svelte Flow's built-in smoothstep edge passes only the endpoints, so pathOptions do not apply.
	const [path] = getSmoothStepPath({
		sourceX: start.x,
		sourceY: start.y,
		sourcePosition: sourceHandle.position,
		targetX: end.x,
		targetY: end.y,
		targetPosition: targetHandle.position,
	});
	return {
		path,
		box: {
			x: Math.min(source.position.x, target.position.x),
			y: Math.min(source.position.y, target.position.y),
			x2: Math.max(source.position.x + source.width, target.position.x + target.width),
			y2: Math.max(source.position.y + source.height, target.position.y + target.height),
		},
	};
}

export type TopologyEdgePulse = { id: string; path: string };

/**
 * The rollout pulse: the paths of the edges that lead to a resource still in progress, the state
 * that shows the activity icon on its card. Edges off a selected path stay dimmed and get none.
 */
export function buildFlowTopologyPulses(
	nodes: readonly FlowTopologyNode[],
	edges: readonly FlowTopologyEdge[],
): TopologyEdgePulse[] {
	const nodesById = new Map<string, FlowTopologyNode>();
	for (const node of nodes) nodesById.set(node.id, node);
	const pulses: TopologyEdgePulse[] = [];
	for (const edge of edges) {
		if (edge.hidden || edge.class === INACTIVE_EDGE_CLASS) continue;
		const source = nodesById.get(edge.source);
		const target = nodesById.get(edge.target);
		const resource = target?.data.node;
		if (!source || !target || source.hidden || target.hidden || !resource) continue;
		if (!resourceLifecycle(resource.kind, resource.status)?.progress) continue;
		const geometry = edgeGeometry(source, target);
		if (geometry) pulses.push({ id: edge.id, path: geometry.path });
	}
	return pulses;
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
