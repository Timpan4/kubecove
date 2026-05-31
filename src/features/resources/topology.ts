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
import {
	buildStandaloneGroups,
	STANDALONE_NODE_WIDTH,
	type StandaloneKindGroupGraphNode,
} from "./topology-standalone-groups";

export type { StandaloneKindGroupGraphNode } from "./topology-standalone-groups";

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
	standalone: boolean;
	showPortHints: boolean;
}

export type OwnershipResourceGraphNode = Node<
	OwnershipGraphNodeData,
	"ownershipResource"
>;

export type OwnershipGraphNode =
	| OwnershipResourceGraphNode
	| StandaloneKindGroupGraphNode;
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

export interface ReactFlowTopologySelectionIndex {
	graph: ReturnType<typeof buildTopologyGraph>;
	nodeIdsByKind: Map<string, string[]>;
}

export interface BuildReactFlowTopologyOptions {
	expandedStandaloneKinds?: ReadonlySet<string>;
	groupStandalone?: boolean;
	showPortHints?: boolean;
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
	options: BuildReactFlowTopologyOptions = {},
): ReactFlowTopology {
	return applyReactFlowTopologySelection(
		buildReactFlowTopologyLayout(topology, selectedNodeId, options),
		topology,
		selectedNodeId,
	);
}

export function buildReactFlowTopologyLayout(
	topology: ResourceTopology,
	selectedNodeIdForExpansion: string | null,
	options: BuildReactFlowTopologyOptions = {},
): ReactFlowTopology {
	const depthById = topologyColumnDepth(topology);
	const graph = buildTopologyGraph(topology);
	const orderById = topologyLayoutOrder(graph.nodes, topology, depthById);
	const positions = buildTopologyPositions(graph, depthById, orderById);
	const compareNodes = compareNodesByLayoutOrder(orderById, depthById);
	const standaloneGroups = options.groupStandalone === false
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
			);
	const sortedNodes = graph.nodes.sort((a, b) => {
		const aStandalone = standaloneGroups.standaloneIds.has(a.id);
		const bStandalone = standaloneGroups.standaloneIds.has(b.id);
		if (aStandalone !== bStandalone) return aStandalone ? 1 : -1;
		const aGroupId = standaloneGroups.groupIdByNodeId.get(a.id);
		const bGroupId = standaloneGroups.groupIdByNodeId.get(b.id);
		if (aGroupId && bGroupId && aGroupId !== bGroupId) {
			return aGroupId.localeCompare(bGroupId);
		}
		const aPosition = positions.get(a.id);
		const bPosition = positions.get(b.id);
		const x = (aPosition?.x ?? 0) - (bPosition?.x ?? 0);
		if (x !== 0) return x;
		const y = (aPosition?.y ?? 0) - (bPosition?.y ?? 0);
		if (y !== 0) return y;
		return compareNodes(a, b);
	}).filter((node) => {
		if (!standaloneGroups.standaloneIds.has(node.id)) return true;
		return standaloneGroups.groupIdByNodeId.has(node.id);
	});
	const nodes = sortedNodes.map<OwnershipGraphNode>((node) => {
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
				resource: topologySelectableResource(node),
				selected: false,
				connected: false,
				dimmed: false,
				standalone,
				showPortHints: options.showPortHints ?? false,
			},
			parentId: standaloneGroups.groupIdByNodeId.get(node.id),
			extent: standalone ? "parent" : undefined,
			draggable: false,
			selectable: true,
			connectable: false,
			selected: false,
			sourcePosition: Position.Right,
			targetPosition: Position.Left,
			style: { width: standalone ? STANDALONE_NODE_WIDTH : NODE_WIDTH },
			zIndex: standalone ? 1 : undefined,
		};
	});
	const edges = graph.edges
		.map<OwnershipGraphEdge>((edge) => {
			return {
				id: edge.id,
				source: edge.source,
				target: edge.target,
				type: "smoothstep",
				data: { relation: edge.relation },
				pathOptions: EDGE_PATH_OPTIONS,
				focusable: false,
				zIndex: 0,
				markerEnd: {
					type: MarkerType.ArrowClosed,
					width: 14,
					height: 14,
					color: "var(--muted-foreground)",
				},
				style: {
					opacity: 0.72,
					stroke: "var(--muted-foreground)",
					strokeWidth: 1.8,
					strokeDasharray:
						edge.relation === "creates" || edge.relation === "selects"
							? "5 5"
							: undefined,
				},
			};
		});

	return { nodes: [...standaloneGroups.groupNodes, ...nodes], edges };
}

export function applyReactFlowTopologySelection(
	graphTopology: ReactFlowTopology,
	topology: ResourceTopology,
	selectedNodeId: string | null,
): ReactFlowTopology {
	return applyReactFlowTopologySelectionWithIndex(
		graphTopology,
		buildReactFlowTopologySelectionIndex(topology),
		selectedNodeId,
	);
}

export function buildReactFlowTopologySelectionIndex(
	topology: ResourceTopology,
): ReactFlowTopologySelectionIndex {
	const graph = buildTopologyGraph(topology);
	const nodeIdsByKind = new Map<string, string[]>();
	for (const node of graph.nodes) {
		const ids = nodeIdsByKind.get(node.kind);
		if (ids) {
			ids.push(node.id);
		} else {
			nodeIdsByKind.set(node.kind, [node.id]);
		}
	}
	return { graph, nodeIdsByKind };
}

export function applyReactFlowTopologySelectionWithIndex(
	graphTopology: ReactFlowTopology,
	index: ReactFlowTopologySelectionIndex,
	selectedNodeId: string | null,
): ReactFlowTopology {
	if (!selectedNodeId) return graphTopology;

	const selectedPath = selectedTopologyPath(index.graph, selectedNodeId);
	const hasSelection = selectedPath.nodeIds.size > 0;
	if (!hasSelection) return graphTopology;

	const nodes = graphTopology.nodes.map<OwnershipGraphNode>((node) => {
		if (node.type === "standaloneKindGroup") {
			const kindNodeIds = index.nodeIdsByKind.get(node.data.kind) ?? [];
			const dimmed = !kindNodeIds.some((nodeId) =>
				selectedPath.nodeIds.has(nodeId),
			);
			return {
				...node,
				data: {
					...node.data,
					dimmed,
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
	const edges = graphTopology.edges.map<OwnershipGraphEdge>((edge) => {
		const selectedEdge = selectedPath.edgeIds.has(edge.id);
		const stroke = selectedEdge ? "var(--primary)" : "var(--muted-foreground)";
		return {
			...edge,
			zIndex: selectedEdge ? 10 : 0,
			markerEnd: {
				type: MarkerType.ArrowClosed,
				width: selectedEdge ? 18 : 14,
				height: selectedEdge ? 18 : 14,
				color: stroke,
			},
			style: {
				...edge.style,
				opacity: selectedEdge ? 1 : 0.16,
				stroke,
				strokeWidth: selectedEdge ? 2.8 : 1.8,
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
	kindSurfaceClassName?: string,
): string {
	const actualNodeId = nodeId ?? node.id ?? null;
	const selected = actualNodeId ? selectedNodeId === actualNodeId : false;
	return cn(
		"resource-topology-node min-w-0 rounded-md border px-3 py-2 text-left shadow-sm transition-all",
		node.selectable
			? "cursor-pointer hover:bg-accent/50 focus-visible:ring-2 focus-visible:ring-ring/40"
			: "cursor-default opacity-90",
		"border-border",
		kindSurfaceClassName,
		node.health === "degraded" && "resource-topology-node-health-degraded",
		node.health === "attention" && "resource-topology-node-health-attention",
		node.health === "restarted" && "resource-topology-node-health-restarted",
		connected && "resource-topology-node-connected ring-1 ring-primary/30",
		selected && "resource-topology-node-selected ring-2 ring-primary",
	);
}

export function topologySelectableResource(
	node: Pick<TopologyNode, "selectable" | "summary">,
): ResourceSummary | null {
	return node.selectable ? node.summary : null;
}
