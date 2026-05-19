import {
	MarkerType,
	Position,
	type Edge,
	type Node,
	type SmoothStepPathOptions,
} from "@xyflow/react";
import type {
	ResourceSummary,
	ResourceTopology,
	TopologyNode,
	TopologyRelation,
} from "@/lib/types";
import { cn } from "@/lib/utils";
import {
	CANVAS_PADDING,
	EDGE_PATH_OPTIONS,
	NODE_WIDTH,
	buildTopologyPositions,
	compareNodesByLayoutOrder,
	sortTopologyNodes,
	topologyColumnDepth,
	topologyLayoutOrder,
} from "./topology-layout";
import {
	buildTopologyGraph,
	selectedTopologyPath,
	uniqueNodes,
} from "./topology-graph";

export interface TopologyRow {
	node: TopologyNode;
	depth: number;
	parentIds: string[];
	incomingRelation: TopologyRelation | null;
}

export interface OwnershipGraphNodeData extends Record<string, unknown> {
	node: TopologyNode;
	resource: ResourceSummary | null;
	selected: boolean;
	connected: boolean;
	dimmed: boolean;
}

export type OwnershipGraphNode = Node<
	OwnershipGraphNodeData,
	"ownershipResource"
>;
export type OwnershipGraphEdge = Edge<
	{ relation: TopologyRelation },
	"smoothstep"
> & {
	pathOptions?: SmoothStepPathOptions;
};

export interface ReactFlowTopology {
	nodes: OwnershipGraphNode[];
	edges: OwnershipGraphEdge[];
}

export function resourceTopologyNodeId(
	cluster: string,
	apiVersion: string,
	kind: string,
	namespace: string | null | undefined,
	name: string,
): string {
	return `${cluster}:${apiVersion}:${kind}:${namespace ?? ""}:${name}`;
}

function sortNodes(a: TopologyNode, b: TopologyNode): number {
	return sortTopologyNodes(a, b);
}

export function buildReactFlowTopology(
	topology: ResourceTopology,
	selectedNodeId: string | null,
): ReactFlowTopology {
	const depthById = topologyColumnDepth(topology);
	const graph = buildTopologyGraph(topology);
	const orderById = topologyLayoutOrder(graph.nodes, topology, depthById);
	const positions = buildTopologyPositions(graph, depthById, orderById);
	const compareNodes = compareNodesByLayoutOrder(orderById, depthById);
	const selectedPath = selectedTopologyPath(graph, selectedNodeId);
	const hasSelection = Boolean(selectedNodeId && selectedPath.nodeIds.size > 0);
	const sortedNodes = graph.nodes.sort((a, b) => {
		const aPosition = positions.get(a.id);
		const bPosition = positions.get(b.id);
		const x = (aPosition?.x ?? 0) - (bPosition?.x ?? 0);
		if (x !== 0) return x;
		const y = (aPosition?.y ?? 0) - (bPosition?.y ?? 0);
		if (y !== 0) return y;
		return compareNodes(a, b);
	});
	const nodes = sortedNodes.map<OwnershipGraphNode>((node) => {
		const selected = selectedNodeId === node.id;
		const connected = selectedPath.nodeIds.has(node.id) && !selected;
		return {
			id: node.id,
			type: "ownershipResource",
			position: positions.get(node.id) ?? { x: CANVAS_PADDING, y: CANVAS_PADDING },
			data: {
				node,
				resource: topologySelectableResource(node),
				selected,
				connected,
				dimmed: hasSelection && !selectedPath.nodeIds.has(node.id),
			},
			draggable: false,
			selectable: true,
			connectable: false,
			selected,
			sourcePosition: Position.Right,
			targetPosition: Position.Left,
			style: { width: NODE_WIDTH },
		};
	});
	const edges = graph.edges
		.map<OwnershipGraphEdge>((edge) => {
			const selectedEdge = selectedPath.edgeIds.has(edge.id);
			const mutedBySelection = Boolean(selectedNodeId && !selectedEdge);
			const stroke = selectedEdge
				? "var(--primary)"
				: "var(--muted-foreground)";
			return {
				id: edge.id,
				source: edge.source,
				target: edge.target,
				type: "smoothstep",
				data: { relation: edge.relation },
				pathOptions: EDGE_PATH_OPTIONS,
				animated: selectedEdge,
				focusable: false,
				zIndex: selectedEdge ? 10 : 0,
				markerEnd: {
					type: MarkerType.ArrowClosed,
					width: selectedEdge ? 18 : 14,
					height: selectedEdge ? 18 : 14,
					color: stroke,
				},
				style: {
					opacity: selectedEdge ? 1 : mutedBySelection ? 0.16 : 0.72,
					stroke,
					strokeWidth: selectedEdge ? 2.8 : 1.8,
					strokeDasharray: edge.relation === "creates" ? "5 5" : undefined,
				},
			};
		});

	return { nodes, edges };
}

export function buildTopologyRows(topology: ResourceTopology): TopologyRow[] {
	const topologyNodes = uniqueNodes(topology.nodes);
	const nodesById = new Map(topologyNodes.map((node) => [node.id, node]));
	const incoming = new Map<string, string[]>();
	const incomingRelation = new Map<string, TopologyRelation>();
	const children = new Map<string, string[]>();

	for (const edge of topology.edges) {
		if (!nodesById.has(edge.source) || !nodesById.has(edge.target)) continue;
		incoming.set(edge.target, [...(incoming.get(edge.target) ?? []), edge.source]);
		incomingRelation.set(edge.target, edge.relation);
		children.set(edge.source, [...(children.get(edge.source) ?? []), edge.target]);
	}

	const rows: TopologyRow[] = [];
	const visited = new Set<string>();
	const sortedNodes = topologyNodes.sort(sortNodes);
	const roots = sortedNodes.filter((node) => !incoming.has(node.id));

	function visit(node: TopologyNode, depth: number, parentIds: string[]) {
		if (visited.has(node.id)) return;
		visited.add(node.id);
		rows.push({
			node,
			depth,
			parentIds,
			incomingRelation: incomingRelation.get(node.id) ?? null,
		});
		const childNodes = (children.get(node.id) ?? [])
			.map((id) => nodesById.get(id))
			.filter((child): child is TopologyNode => Boolean(child))
			.sort(sortNodes);
		for (const child of childNodes) {
			visit(child, depth + 1, [...parentIds, node.id]);
		}
	}

	for (const root of roots) visit(root, 0, []);
	for (const node of sortedNodes) visit(node, 0, []);
	return rows;
}

export function topologyNodeClassName(
	node: Pick<TopologyNode, "health" | "selectable"> & { id?: string },
	selectedNodeId: string | null,
	nodeId?: string,
	connected = false,
): string {
	const actualNodeId = nodeId ?? node.id ?? null;
	const selected = actualNodeId ? selectedNodeId === actualNodeId : false;
	return cn(
		"min-w-0 rounded-md border bg-card px-3 py-2 text-left shadow-sm transition-all",
		node.selectable
			? "cursor-pointer hover:bg-accent/50 focus-visible:ring-2 focus-visible:ring-ring/40"
			: "cursor-default opacity-90",
		"border-border",
		node.health === "degraded" && "border-red-500 bg-red-500/10",
		node.health === "attention" && "border-amber-500/70 bg-amber-500/10",
		node.health === "restarted" && "border-sky-500/70 bg-sky-500/10",
		connected &&
			"border-primary/60 bg-primary/5 ring-1 ring-primary/30 shadow-primary/10",
		selected &&
			"border-primary bg-primary/10 ring-2 ring-primary shadow-lg shadow-primary/25",
	);
}

export function topologySelectableResource(
	node: Pick<TopologyNode, "selectable" | "summary">,
): ResourceSummary | null {
	return node.selectable ? node.summary : null;
}
