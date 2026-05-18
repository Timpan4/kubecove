import {
	MarkerType,
	Position,
	type Edge,
	type Node,
} from "@xyflow/react";
import type {
	ResourceSummary,
	ResourceTopology,
	TopologyNode,
	TopologyRelation,
} from "@/lib/types";
import { cn } from "@/lib/utils";

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
}

export type OwnershipGraphNode = Node<
	OwnershipGraphNodeData,
	"ownershipResource"
>;
export type OwnershipGraphEdge = Edge<{ relation: TopologyRelation }>;

export interface ReactFlowTopology {
	nodes: OwnershipGraphNode[];
	edges: OwnershipGraphEdge[];
}

const NODE_WIDTH = 190;
const COLUMN_GAP = 104;
const ROW_GAP = 34;
const NODE_HEIGHT = 66;
const CANVAS_PADDING = 180;

const KIND_RANK: Record<string, number> = {
	Deployment: 0,
	DaemonSet: 0,
	StatefulSet: 0,
	CronJob: 0,
	Service: 0,
	Ingress: 0,
	ConfigMap: 0,
	Secret: 0,
	ReplicaSet: 1,
	Job: 1,
	Pod: 2,
	PersistentVolumeClaim: 3,
};

function kindRank(kind: string): number {
	return KIND_RANK[kind] ?? 9;
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
	const rank = kindRank(a.kind) - kindRank(b.kind);
	if (rank !== 0) return rank;
	const namespace = (a.namespace ?? "").localeCompare(b.namespace ?? "");
	if (namespace !== 0) return namespace;
	const kind = a.kind.localeCompare(b.kind);
	if (kind !== 0) return kind;
	return a.name.localeCompare(b.name, undefined, { numeric: true });
}

function uniqueNodes(nodes: TopologyNode[]): TopologyNode[] {
	return Array.from(new Map(nodes.map((node) => [node.id, node])).values());
}

function topologyColumnDepth(topology: ResourceTopology): Map<string, number> {
	const topologyNodes = uniqueNodes(topology.nodes);
	const nodesById = new Map(topologyNodes.map((node) => [node.id, node]));
	const parents = new Map<string, string[]>();
	for (const edge of topology.edges) {
		if (!nodesById.has(edge.source) || !nodesById.has(edge.target)) continue;
		parents.set(edge.target, [...(parents.get(edge.target) ?? []), edge.source]);
	}

	const depthById = new Map<string, number>();
	const visiting = new Set<string>();
	function resolve(node: TopologyNode): number {
		const cached = depthById.get(node.id);
		if (cached !== undefined) return cached;
		if (visiting.has(node.id)) return kindRank(node.kind);
		visiting.add(node.id);
		const parentIds = parents.get(node.id) ?? [];
		if (parentIds.length === 0) {
			depthById.set(node.id, 0);
			visiting.delete(node.id);
			return 0;
		}
		const parentDepths = parentIds
			.map((id) => nodesById.get(id))
			.filter((parent): parent is TopologyNode => Boolean(parent))
			.map((parent) => resolve(parent) + 1);
		const edgeDepth = Math.max(...parentDepths);
		const depth = Math.max(edgeDepth, kindRank(node.kind));
		depthById.set(node.id, depth);
		visiting.delete(node.id);
		return depth;
	}

	for (const node of topologyNodes) resolve(node);
	return depthById;
}

function topologyIncomingParents(topology: ResourceTopology): Map<string, string[]> {
	const nodeIds = new Set(uniqueNodes(topology.nodes).map((node) => node.id));
	const parents = new Map<string, string[]>();
	for (const edge of topology.edges) {
		if (!nodeIds.has(edge.source) || !nodeIds.has(edge.target)) continue;
		parents.set(edge.target, [...(parents.get(edge.target) ?? []), edge.source]);
	}
	return parents;
}

function topologyLayoutOrder(
	nodes: TopologyNode[],
	topology: ResourceTopology,
	depthById: Map<string, number>,
): Map<string, string> {
	const nodesById = new Map(nodes.map((node) => [node.id, node]));
	const parents = topologyIncomingParents(topology);
	const roots = nodes
		.filter((node) => (parents.get(node.id) ?? []).length === 0)
		.sort(sortNodes);
	const rootOrder = new Map(roots.map((node, index) => [node.id, index]));
	const memo = new Map<string, string>();
	const visiting = new Set<string>();
	const localKey = (node: TopologyNode) =>
		[
			String(kindRank(node.kind)).padStart(2, "0"),
			node.namespace ?? "",
			node.kind,
			node.name,
			node.id,
		].join("|");

	function resolve(node: TopologyNode): string {
		const cached = memo.get(node.id);
		if (cached) return cached;
		if (visiting.has(node.id)) return localKey(node);
		visiting.add(node.id);
		const parentNodes = (parents.get(node.id) ?? [])
			.map((id) => nodesById.get(id))
			.filter((parent): parent is TopologyNode => Boolean(parent))
			.sort((a, b) => sortNodes(a, b));
		const parentKey =
			parentNodes.length > 0
				? parentNodes.map(resolve).sort()[0]
				: String(rootOrder.get(node.id) ?? roots.length).padStart(4, "0");
		const key = [
			parentKey,
			String(depthById.get(node.id) ?? 0).padStart(2, "0"),
			localKey(node),
		].join(">");
		memo.set(node.id, key);
		visiting.delete(node.id);
		return key;
	}

	for (const node of nodes) resolve(node);
	return memo;
}

export function buildReactFlowTopology(
	topology: ResourceTopology,
	selectedNodeId: string | null,
): ReactFlowTopology {
	const depthById = topologyColumnDepth(topology);
	const topologyNodes = uniqueNodes(topology.nodes);
	const orderById = topologyLayoutOrder(topologyNodes, topology, depthById);
	const sortedNodes = topologyNodes.sort((a, b) => {
		const column = (depthById.get(a.id) ?? 0) - (depthById.get(b.id) ?? 0);
		if (column !== 0) return column;
		const order = (orderById.get(a.id) ?? "").localeCompare(
			orderById.get(b.id) ?? "",
		);
		if (order !== 0) return order;
		return sortNodes(a, b);
	});
	const columnRows = new Map<number, number>();
	const selectedConnections = new Set<string>();
	if (selectedNodeId) {
		for (const edge of topology.edges) {
			if (edge.source === selectedNodeId) selectedConnections.add(edge.target);
			if (edge.target === selectedNodeId) selectedConnections.add(edge.source);
		}
	}
	const nodes = sortedNodes.map<OwnershipGraphNode>((node) => {
		const column = depthById.get(node.id) ?? kindRank(node.kind);
		const row = columnRows.get(column) ?? 0;
		const selected = selectedNodeId === node.id;
		columnRows.set(column, row + 1);
		return {
			id: node.id,
			type: "ownershipResource",
			position: {
				x: CANVAS_PADDING + column * (NODE_WIDTH + COLUMN_GAP),
				y: CANVAS_PADDING + row * (NODE_HEIGHT + ROW_GAP),
			},
			data: {
				node,
				resource: topologySelectableResource(node),
				selected,
				connected: selectedConnections.has(node.id),
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
	const nodeIds = new Set(nodes.map((node) => node.id));
	const uniqueEdges = Array.from(
		new Map(topology.edges.map((edge) => [edge.id, edge])).values(),
	);
	const edges = uniqueEdges
		.filter((edge) => nodeIds.has(edge.source) && nodeIds.has(edge.target))
		.map<OwnershipGraphEdge>((edge) => {
			const selectedEdge = Boolean(
				selectedNodeId &&
					(edge.source === selectedNodeId || edge.target === selectedNodeId),
			);
			const mutedBySelection = Boolean(selectedNodeId && !selectedEdge);
			const stroke = selectedEdge
				? "var(--primary)"
				: "var(--muted-foreground)";
			return {
				id: edge.id,
				source: edge.source,
				target: edge.target,
				type: "simplebezier",
				data: { relation: edge.relation },
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
					opacity: selectedEdge ? 1 : mutedBySelection ? 0.28 : 0.72,
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
