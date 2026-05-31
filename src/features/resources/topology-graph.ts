import type { ResourceTopology, TopologyNode } from "@/lib/types";

export type TopologyRelationEdge = ResourceTopology["edges"][number];

export interface TopologyGraph {
	nodes: TopologyNode[];
	nodeIds: Set<string>;
	nodesById: Map<string, TopologyNode>;
	edges: TopologyRelationEdge[];
	parents: Map<string, string[]>;
	children: Map<string, string[]>;
	incomingEdges: Map<string, TopologyRelationEdge[]>;
	outgoingEdges: Map<string, TopologyRelationEdge[]>;
}

function pushMapValue<K, V>(map: Map<K, V[]>, key: K, value: V): void {
	const values = map.get(key);
	if (values) {
		values.push(value);
		return;
	}
	map.set(key, [value]);
}

export function uniqueNodes(nodes: TopologyNode[]): TopologyNode[] {
	return Array.from(new Map(nodes.map((node) => [node.id, node])).values());
}

export function buildTopologyGraph(topology: ResourceTopology): TopologyGraph {
	const nodes = uniqueNodes(topology.nodes);
	const nodeIds = new Set(nodes.map((node) => node.id));
	const nodesById = new Map(nodes.map((node) => [node.id, node]));
	const edges = Array.from(
		new Map(topology.edges.map((edge) => [edge.id, edge])).values(),
	).filter((edge) => nodeIds.has(edge.source) && nodeIds.has(edge.target));
	const parents = new Map<string, string[]>();
	const children = new Map<string, string[]>();
	const incomingEdges = new Map<string, TopologyRelationEdge[]>();
	const outgoingEdges = new Map<string, TopologyRelationEdge[]>();

	for (const edge of edges) {
		pushMapValue(parents, edge.target, edge.source);
		pushMapValue(children, edge.source, edge.target);
		pushMapValue(incomingEdges, edge.target, edge);
		pushMapValue(outgoingEdges, edge.source, edge);
	}

	return {
		nodes,
		nodeIds,
		nodesById,
		edges,
		parents,
		children,
		incomingEdges,
		outgoingEdges,
	};
}

export function selectedTopologyPath(
	graph: TopologyGraph,
	selectedNodeId: string | null,
): { nodeIds: Set<string>; edgeIds: Set<string> } {
	const nodeIds = new Set<string>();
	const edgeIds = new Set<string>();
	if (!selectedNodeId || !graph.nodeIds.has(selectedNodeId)) {
		return { nodeIds, edgeIds };
	}

	nodeIds.add(selectedNodeId);
	const visitAncestors = (nodeId: string) => {
		for (const edge of graph.incomingEdges.get(nodeId) ?? []) {
			if (edgeIds.has(edge.id)) continue;
			edgeIds.add(edge.id);
			if (!nodeIds.has(edge.source)) {
				nodeIds.add(edge.source);
				visitAncestors(edge.source);
			}
		}
	};
	const visitDescendants = (nodeId: string) => {
		for (const edge of graph.outgoingEdges.get(nodeId) ?? []) {
			if (edgeIds.has(edge.id)) continue;
			edgeIds.add(edge.id);
			if (!nodeIds.has(edge.target)) {
				nodeIds.add(edge.target);
				visitDescendants(edge.target);
			}
		}
	};

	visitAncestors(selectedNodeId);
	visitDescendants(selectedNodeId);
	return { nodeIds, edgeIds };
}
