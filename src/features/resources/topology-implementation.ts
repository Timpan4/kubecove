import {
	type CoordinateExtent,
	MarkerType,
	Position,
} from "@xyflow/system";
import type {
	ResourceTopology,
	TopologyMode,
} from "@/lib/types";
import {
	buildTopologyGraph,
	selectedTopologyPath,
	selectedTopologyRootSubtree,
} from "./topology-graph";
import {
	buildTopologyPositions,
	CANVAS_PADDING,
	compareNodesByLayoutOrder,
	EDGE_PATH_OPTIONS,
	NODE_HEIGHT,
	NODE_WIDTH,
	topologyColumnDepth,
	topologyLayoutOrder,
} from "./topology-layout";

import { buildStandaloneGroups } from "./topology-standalone-groups";
import type {
	BuildFlowTopologyOptions,
	FlowTopology,
	FlowTopologyBounds,
	FlowTopologyEdge,
	FlowTopologyNode,
	FlowTopologySelectionIndex,
	OwnershipMapViewportSize,
} from "./topology-types";

export { resourceTopologyNodeId } from "./topology-graph";
export type {
	BuildFlowTopologyOptions,
	FlowTopology,
	FlowTopologyBounds,
	FlowTopologyEdge,
	FlowTopologyNode,
	FlowTopologyNodeData,
	FlowTopologySelectionIndex,
} from "./topology-types";

const SVELTE_TOPOLOGY_LAYOUT = {
	nodeWidth: 408,
	nodeHeight: 98,
	columnGap: 112,
	rowGap: 30,
};
const SVELTE_STANDALONE_NODE_WIDTH = 320;
const SVELTE_MIN_ZOOM = 0.12;
const SVELTE_MAX_ZOOM = 1.4;
const WIDTH_FIT_PADDING = 0.08;
const PAN_PADDING_RATIO = 0.42;
const MIN_PAN_PADDING = 180;
const MAX_PAN_PADDING = 720;

function ownershipMapBoundaryPadding(
	viewportSize: OwnershipMapViewportSize,
): { x: number; y: number } {
	return {
		x: Math.min(
			MAX_PAN_PADDING,
			Math.max(MIN_PAN_PADDING, viewportSize.width * PAN_PADDING_RATIO),
		),
		y: Math.min(
			MAX_PAN_PADDING,
			Math.max(MIN_PAN_PADDING, viewportSize.height * PAN_PADDING_RATIO),
		),
	};
}
const ZERO_TRANSLATE_EXTENT: CoordinateExtent = [
	[0, 0],
	[0, 0],
];

function finitePositiveNumber(value: unknown): number | null {
	if (typeof value !== "number" || !Number.isFinite(value) || value <= 0) return null;
	return value;
}

function nodeDimension(node: FlowTopologyNode, dimension: "width" | "height"): number {
	return (
		finitePositiveNumber(node[dimension]) ??
		finitePositiveNumber(node.measured?.[dimension]) ??
		(dimension === "width" ? NODE_WIDTH : NODE_HEIGHT)
	);
}

function absoluteTopologyNodePosition(
	nodesById: ReadonlyMap<string, FlowTopologyNode>,
	node: FlowTopologyNode,
): { x: number; y: number } {
	let x = node.position.x;
	let y = node.position.y;
	let parentId = node.parentId;
	const visitedParentIds = new Set<string>();

	while (parentId) {
		if (visitedParentIds.has(parentId)) break;
		visitedParentIds.add(parentId);
		const parent = nodesById.get(parentId);
		if (!parent) break;
		x += parent.position.x;
		y += parent.position.y;
		parentId = parent.parentId;
	}

	return { x, y };
}

function clampMapZoom(zoom: number): number {
	return Math.min(SVELTE_MAX_ZOOM, Math.max(SVELTE_MIN_ZOOM, zoom));
}

function compactDirectEdgeDepths(
	graph: ReturnType<typeof buildTopologyGraph>,
	depthById: Map<string, number>,
): Map<string, number> {
	const compacted = new Map(depthById);
	const nodesByDepth = [...graph.nodes].sort(
		(a, b) => (depthById.get(a.id) ?? 0) - (depthById.get(b.id) ?? 0),
	);
	for (const node of nodesByDepth) {
		const parentIds = graph.parents.get(node.id) ?? [];
		if (parentIds.length === 0) continue;
		const directDepth = Math.max(
			...parentIds.map((id) => (compacted.get(id) ?? 0) + 1),
		);
		const currentDepth = compacted.get(node.id) ?? directDepth;
		if (directDepth < currentDepth) compacted.set(node.id, directDepth);
	}
	return compacted;
}

export function getFlowTopologyBounds(
	nodes: FlowTopologyNode[],
	nodesToBound: FlowTopologyNode[] = nodes,
): FlowTopologyBounds | null {
	if (nodesToBound.length === 0) return null;

	const nodesById = new Map(nodes.map((node) => [node.id, node]));
	let left = Number.POSITIVE_INFINITY;
	let top = Number.POSITIVE_INFINITY;
	let right = Number.NEGATIVE_INFINITY;
	let bottom = Number.NEGATIVE_INFINITY;

	for (const node of nodesToBound) {
		const position = absoluteTopologyNodePosition(nodesById, node);
		left = Math.min(left, position.x);
		top = Math.min(top, position.y);
		right = Math.max(right, position.x + nodeDimension(node, "width"));
		bottom = Math.max(bottom, position.y + nodeDimension(node, "height"));
	}

	if (!Number.isFinite(left + top + right + bottom)) return null;
	return { left, top, right, bottom, width: right - left, height: bottom - top };
}

export function widthFitFlowTopologyViewport(
	bounds: FlowTopologyBounds,
	viewportSize: OwnershipMapViewportSize,
): { x: number; y: number; zoom: number } {
	const usableWidth = Math.max(1, viewportSize.width * (1 - WIDTH_FIT_PADDING * 2));
	const zoom = clampMapZoom(usableWidth / Math.max(1, bounds.width));
	const centerX = bounds.left + bounds.width / 2;
	const centerY = bounds.top + bounds.height / 2;

	return {
		x: viewportSize.width / 2 - centerX * zoom,
		y: viewportSize.height / 2 - centerY * zoom,
		zoom,
	};
}

export function buildFlowTopology(
	topology: ResourceTopology,
	selectedNodeId: string | null,
	mode: TopologyMode = "ownership",
	options: BuildFlowTopologyOptions = {},
): FlowTopology {
	return applyFlowTopologySelectionWithIndex(
		buildFlowTopologyLayout(topology, selectedNodeId, mode, options),
		buildFlowTopologySelectionIndex(topology),
		selectedNodeId,
	);
}

export function buildFlowTopologyLayout(
	topology: ResourceTopology,
	selectedNodeIdForExpansion: string | null,
	mode: TopologyMode = "ownership",
	options: BuildFlowTopologyOptions = {},
): FlowTopology {
	const graph = buildTopologyGraph(topology);
	const depthById = compactDirectEdgeDepths(graph, topologyColumnDepth(topology));
	const orderById = topologyLayoutOrder(graph.nodes, topology, depthById);
	const positions = buildTopologyPositions(
		graph,
		depthById,
		orderById,
		SVELTE_TOPOLOGY_LAYOUT,
	);
	const compareNodes = compareNodesByLayoutOrder(orderById, depthById);
	const standaloneGroups =
		mode === "networkFlow"
			? {
					groupNodes: [],
					standaloneIds: new Set<string>(),
					groupIdByNodeId: new Map<string, string>(),
					positionsById: new Map<string, { x: number; y: number }>(),
				}
			: buildStandaloneGroups(
					graph,
					positions,
					compareNodes,
					new Set<string>(),
					false,
					options.expandedStandaloneKinds ?? new Set<string>(),
					selectedNodeIdForExpansion,
					{
						nodeHeight: SVELTE_TOPOLOGY_LAYOUT.nodeHeight,
						standaloneNodeWidth: SVELTE_STANDALONE_NODE_WIDTH,
				},
			);
	const groupNodes = standaloneGroups.groupNodes.map<FlowTopologyNode>((group) => {
		const width = group.style?.width ?? SVELTE_TOPOLOGY_LAYOUT.nodeWidth;
		const height = group.style?.height ?? SVELTE_TOPOLOGY_LAYOUT.nodeHeight;
		return {
			id: group.id,
			type: "standaloneKindGroup",
			position: group.position,
			data: {
				resource: null,
				label: group.data.kind,
				selected: false,
				connected: false,
				kind: group.data.kind,
				count: group.data.count,
				nodeIds: group.data.nodeIds,
				expanded: group.data.expanded,
				dimmed: group.data.dimmed,
			},
			draggable: false,
			connectable: false,
			focusable: false,
			selectable: true,
			width,
			height,
			style: `width: ${width}px; height: ${height}px`,
			zIndex: group.zIndex,
		};
	});
	const nodes = [...graph.nodes].sort(compareNodes).filter((node) => {
		if (!standaloneGroups.standaloneIds.has(node.id)) return true;
		return standaloneGroups.groupIdByNodeId.has(node.id);
	}).map<FlowTopologyNode>((node) => {
		const standalone = standaloneGroups.groupIdByNodeId.has(node.id);
		return {
			id: node.id,
			type: "ownershipResource",
			position:
				standaloneGroups.positionsById.get(node.id) ??
				positions.get(node.id) ??
				{ x: CANVAS_PADDING, y: CANVAS_PADDING },
			data: {
				node,
				resource: node.selectable ? node.summary : null,
				label: node.name,
				selected: false,
				connected: false,
				dimmed: false,
				showPortHints: mode === "networkFlow",
			},
			selected: false,
			draggable: false,
			connectable: false,
			focusable: true,
			parentId: standaloneGroups.groupIdByNodeId.get(node.id),
			extent: standalone ? "parent" : undefined,
			sourcePosition: Position.Right,
			targetPosition: Position.Left,
			width: standalone ? SVELTE_STANDALONE_NODE_WIDTH : SVELTE_TOPOLOGY_LAYOUT.nodeWidth,
			height: SVELTE_TOPOLOGY_LAYOUT.nodeHeight,
			style: `width: ${
				standalone ? SVELTE_STANDALONE_NODE_WIDTH : SVELTE_TOPOLOGY_LAYOUT.nodeWidth
			}px; height: ${SVELTE_TOPOLOGY_LAYOUT.nodeHeight}px`,
			zIndex: standalone ? 1 : undefined,
		};
	});
	const edges = graph.edges.map<FlowTopologyEdge>((edge) => {
		return {
			id: edge.id,
			source: edge.source,
			target: edge.target,
			type: "smoothstep",
			data: { relation: edge.relation },
			pathOptions: EDGE_PATH_OPTIONS,
			animated: false,
			focusable: false,
			markerEnd: {
				type: MarkerType.ArrowClosed,
				width: 14,
				height: 14,
				color: "var(--muted-foreground)",
			},
			style: [
				"stroke: var(--muted-foreground)",
				"stroke-width: 1.6",
				"opacity: 0.62",
			].join("; "),
		};
	});
	return { nodes: [...groupNodes, ...nodes], edges };
}

export function buildFlowTopologySelectionIndex(
	topology: ResourceTopology,
): FlowTopologySelectionIndex {
	return { graph: buildTopologyGraph(topology) };
}

export function applyFlowTopologySelectionWithIndex(
	graphTopology: FlowTopology,
	index: FlowTopologySelectionIndex,
	selectedNodeId: string | null,
): FlowTopology {
	if (!selectedNodeId) return graphTopology;

	const selectedPath = selectedTopologyPath(index.graph, selectedNodeId);
	if (selectedPath.nodeIds.size === 0) return graphTopology;

	const nodes = graphTopology.nodes.map<FlowTopologyNode>((node) => {
		if (node.type === "standaloneKindGroup") {
			const nodeIds = node.data.nodeIds ?? [];
			return {
				...node,
				data: {
					...node.data,
					dimmed: !nodeIds.some((nodeId) => selectedPath.nodeIds.has(nodeId)),
				},
			};
		}

		const selected = selectedNodeId === node.id;
		const connected = selectedPath.nodeIds.has(node.id) && !selected;
		return {
			...node,
			data: {
				...node.data,
				selected,
				connected,
				dimmed: !selectedPath.nodeIds.has(node.id),
			},
			selected,
		};
	});
	const edges = graphTopology.edges.map<FlowTopologyEdge>((edge) => {
		const active = selectedPath.edgeIds.has(edge.id);
		const stroke = active ? "var(--primary)" : "var(--muted-foreground)";
		return {
			...edge,
			animated: active,
			markerEnd: {
				type: MarkerType.ArrowClosed,
				width: active ? 18 : 14,
				height: active ? 18 : 14,
				color: stroke,
			},
			style: [
				`stroke: ${stroke}`,
				`stroke-width: ${active ? 2.6 : 1.6}`,
				`opacity: ${active ? 1 : 0.2}`,
			].join("; "),
		};
	});

	return { nodes, edges };
}

export function filterFlowTopologyToSelectedRoot(
	graphTopology: FlowTopology,
	index: FlowTopologySelectionIndex,
	selectedNodeId: string | null,
): FlowTopology {
	if (!selectedNodeId) return graphTopology;

	const rootSubtree = selectedTopologyRootSubtree(index.graph, selectedNodeId);
	if (rootSubtree.nodeIds.size === 0) return graphTopology;

	const graphNodeIds = new Set(rootSubtree.nodeIds);
	for (const node of graphTopology.nodes) {
		const nodeIds = node.data.nodeIds ?? [];
		if (nodeIds.some((nodeId) => rootSubtree.nodeIds.has(nodeId))) {
			graphNodeIds.add(node.id);
		}
	}

	const nodes = graphTopology.nodes.filter((node) => graphNodeIds.has(node.id));
	const visibleNodeIds = new Set(nodes.map((node) => node.id));
	const edges = graphTopology.edges.filter(
		(edge) =>
			rootSubtree.edgeIds.has(edge.id) &&
			visibleNodeIds.has(edge.source) &&
			visibleNodeIds.has(edge.target),
	);

	return { nodes, edges };
}

export function getTopologyTranslateExtent(
	nodes: FlowTopologyNode[],
	viewportSize: OwnershipMapViewportSize,
): CoordinateExtent {
	if (viewportSize.width <= 0 || viewportSize.height <= 0 || nodes.length === 0) {
		return ZERO_TRANSLATE_EXTENT;
	}

	const padding = ownershipMapBoundaryPadding(viewportSize);
	const nodesById = new Map(nodes.map((node) => [node.id, node]));
	let left = Number.POSITIVE_INFINITY;
	let top = Number.POSITIVE_INFINITY;
	let right = Number.NEGATIVE_INFINITY;
	let bottom = Number.NEGATIVE_INFINITY;

	for (const node of nodes) {
		const position = absoluteTopologyNodePosition(nodesById, node);
		left = Math.min(left, position.x);
		top = Math.min(top, position.y);
		right = Math.max(right, position.x + nodeDimension(node, "width"));
		bottom = Math.max(bottom, position.y + nodeDimension(node, "height"));
	}

	if (![left, top, right, bottom].every(Number.isFinite)) return ZERO_TRANSLATE_EXTENT;
	const zoomSlack = {
		x: viewportSize.width / SVELTE_MIN_ZOOM,
		y: viewportSize.height / SVELTE_MIN_ZOOM,
	};
	return [
		[left - padding.x - zoomSlack.x, top - padding.y - zoomSlack.y],
		[right + padding.x + zoomSlack.x, bottom + padding.y + zoomSlack.y],
	];
}

export function topologyViewportFitKey(
	nodes: FlowTopologyNode[],
	nodesToFit: FlowTopologyNode[],
	edges: FlowTopologyEdge[],
	selectedNodeId: string | null,
	viewportKey: string,
): string {
	const nodesById = new Map(nodes.map((node) => [node.id, node]));
	const fittingSelection = nodesToFit.some((node) => node.data.selected || node.data.connected);
	const nodeParts = nodesToFit
		.map((node) => {
			const position = absoluteTopologyNodePosition(nodesById, node);
			return [
				node.id,
				node.parentId ?? "",
				node.type ?? "",
				position.x,
				position.y,
				nodeDimension(node, "width"),
				nodeDimension(node, "height"),
			].join(":");
		})
		.sort();
	const edgeParts = edges.map((edge) => edge.id).sort();
	return [
		fittingSelection ? "selected" : "all",
		selectedNodeId ?? "",
		viewportKey,
		nodeParts.join("|"),
		fittingSelection ? "" : edgeParts.join("|"),
	].join("::");
}

export interface FlowTopologyFitPlan {
	key: string;
	viewport: { x: number; y: number; zoom: number };
	focused: boolean;
}

export function buildFlowTopologyFitPlan(
	nodes: FlowTopologyNode[],
	edges: FlowTopologyEdge[],
	selectedNodeId: string | null,
	viewportKey: string,
	viewportSize: OwnershipMapViewportSize,
): FlowTopologyFitPlan | null {
	const selectedPathNodes = selectedNodeId
		? nodes.filter((node) => node.data.selected || node.data.connected)
		: [];
	const nodesToFit = selectedPathNodes.length > 0 ? selectedPathNodes : nodes;
	const bounds = getFlowTopologyBounds(nodes, nodesToFit);
	if (!bounds || viewportSize.width <= 0 || viewportSize.height <= 0) return null;
	return {
		key: topologyViewportFitKey(
			nodes,
			nodesToFit,
			edges,
			selectedNodeId,
			viewportKey,
		),
		viewport: widthFitFlowTopologyViewport(bounds, viewportSize),
		focused: selectedPathNodes.length > 0,
	};
}
