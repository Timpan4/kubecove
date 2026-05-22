import type { SmoothStepPathOptions } from "@xyflow/react";
import type { ResourceTopology, TopologyNode } from "@/lib/types";
import { type TopologyGraph, uniqueNodes } from "./topology-graph";

export const NODE_WIDTH = 190;
export const NODE_HEIGHT = 78;
const COLUMN_GAP = 104;
const ROW_GAP = 14;
export const CANVAS_PADDING = 96;
const NODE_Y_STEP = NODE_HEIGHT + ROW_GAP;
const SUBTREE_GAP = 20;
const ROOT_GROUP_GAP = 16;
const STANDALONE_GROUP_GAP = 10;

export const EDGE_PATH_OPTIONS: SmoothStepPathOptions = {
	borderRadius: 10,
	offset: 22,
	stepPosition: 0.42,
};

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
	EndpointSlice: 2,
	Pod: 2,
	PersistentVolumeClaim: 3,
};

export function kindRank(kind: string): number {
	return KIND_RANK[kind] ?? 9;
}

function createdAtMs(node: TopologyNode): number | null {
	const createdAt = node.summary.createdAt;
	if (!createdAt) return null;
	const ms = Date.parse(createdAt);
	return Number.isFinite(ms) ? ms : null;
}

function compareCreatedAtNewestFirst(a: TopologyNode, b: TopologyNode): number {
	const aMs = createdAtMs(a);
	const bMs = createdAtMs(b);
	if (aMs === null && bMs === null) return 0;
	if (aMs === null) return 1;
	if (bMs === null) return -1;
	return bMs - aMs;
}

function createdAtNewestFirstKey(node: TopologyNode): string {
	const ms = createdAtMs(node);
	if (ms === null) return "1:";
	return `0:${String(Number.MAX_SAFE_INTEGER - ms).padStart(16, "0")}`;
}

export function sortTopologyNodes(a: TopologyNode, b: TopologyNode): number {
	const rank = kindRank(a.kind) - kindRank(b.kind);
	if (rank !== 0) return rank;
	const namespace = (a.namespace ?? "").localeCompare(b.namespace ?? "");
	if (namespace !== 0) return namespace;
	const kind = a.kind.localeCompare(b.kind);
	if (kind !== 0) return kind;
	const createdAt = compareCreatedAtNewestFirst(a, b);
	if (createdAt !== 0) return createdAt;
	return a.name.localeCompare(b.name, undefined, { numeric: true });
}

export function topologyColumnDepth(
	topology: ResourceTopology,
): Map<string, number> {
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

export function topologyLayoutOrder(
	nodes: TopologyNode[],
	topology: ResourceTopology,
	depthById: Map<string, number>,
): Map<string, string> {
	const nodesById = new Map(nodes.map((node) => [node.id, node]));
	const parents = topologyIncomingParents(topology);
	const roots = nodes
		.filter((node) => (parents.get(node.id) ?? []).length === 0)
		.sort(sortTopologyNodes);
	const rootOrder = new Map(roots.map((node, index) => [node.id, index]));
	const memo = new Map<string, string>();
	const visiting = new Set<string>();
	const localKey = (node: TopologyNode) =>
		[
			String(kindRank(node.kind)).padStart(2, "0"),
			node.namespace ?? "",
			node.kind,
			createdAtNewestFirstKey(node),
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
			.sort(sortTopologyNodes);
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

export function compareNodesByLayoutOrder(
	orderById: Map<string, string>,
	depthById: Map<string, number>,
): (a: TopologyNode, b: TopologyNode) => number {
	return (a, b) => {
		const depth = (depthById.get(a.id) ?? 0) - (depthById.get(b.id) ?? 0);
		if (depth !== 0) return depth;
		const order = (orderById.get(a.id) ?? "").localeCompare(
			orderById.get(b.id) ?? "",
		);
		if (order !== 0) return order;
		return sortTopologyNodes(a, b);
	};
}

function buildPrimaryChildren(
	graph: TopologyGraph,
	orderById: Map<string, string>,
	depthById: Map<string, number>,
): Map<string, string[]> {
	const primaryChildren = new Map<string, string[]>();
	const compareNodes = compareNodesByLayoutOrder(orderById, depthById);

	for (const node of graph.nodes) {
		const parentNodes = (graph.parents.get(node.id) ?? [])
			.map((id) => graph.nodesById.get(id))
			.filter((parent): parent is TopologyNode => Boolean(parent))
			.sort(compareNodes);
		const primaryParent = parentNodes[0];
		if (!primaryParent) continue;
		primaryChildren.set(primaryParent.id, [
			...(primaryChildren.get(primaryParent.id) ?? []),
			node.id,
		]);
	}

	for (const [parentId, childIds] of primaryChildren.entries()) {
		const sortedChildIds = childIds
			.map((id) => graph.nodesById.get(id))
			.filter((node): node is TopologyNode => Boolean(node))
			.sort(compareNodes)
			.map((node) => node.id);
		primaryChildren.set(parentId, sortedChildIds);
	}

	return primaryChildren;
}

export function buildTopologyPositions(
	graph: TopologyGraph,
	depthById: Map<string, number>,
	orderById: Map<string, string>,
): Map<string, { x: number; y: number }> {
	const primaryChildren = buildPrimaryChildren(graph, orderById, depthById);
	const primaryChildIds = new Set(
		Array.from(primaryChildren.values()).flatMap((childIds) => childIds),
	);
	const compareNodes = compareNodesByLayoutOrder(orderById, depthById);
	const roots = graph.nodes
		.filter((node) => !primaryChildIds.has(node.id))
		.sort(compareNodes);
	const ownedRoots = roots.filter(
		(node) => (primaryChildren.get(node.id) ?? []).length > 0,
	);
	const standaloneRoots = roots.filter(
		(node) => (primaryChildren.get(node.id) ?? []).length === 0,
	);
	const orderedRoots =
		roots.length > 0 ? [...ownedRoots, ...standaloneRoots] : graph.nodes.sort(compareNodes);
	const positions = new Map<string, { x: number; y: number }>();
	const visited = new Set<string>();

	function nodeX(node: TopologyNode): number {
		const column = depthById.get(node.id) ?? kindRank(node.kind);
		return CANVAS_PADDING + column * (NODE_WIDTH + COLUMN_GAP);
	}

	function layoutNode(nodeId: string, topY: number, visiting: Set<string>): number {
		const node = graph.nodesById.get(nodeId);
		if (!node || visited.has(nodeId)) return topY;
		if (visiting.has(nodeId)) {
			positions.set(nodeId, { x: nodeX(node), y: topY });
			visited.add(nodeId);
			return topY + NODE_Y_STEP;
		}

		visiting.add(nodeId);
		const childIds = primaryChildren.get(nodeId) ?? [];
		const placedChildYs: number[] = [];
		let nextY = topY;

		for (const childId of childIds) {
			const beforeChildY = nextY;
			nextY = layoutNode(childId, nextY, visiting);
			const childY = positions.get(childId)?.y ?? beforeChildY;
			placedChildYs.push(childY);
			const childHasChildren = (primaryChildren.get(childId) ?? []).length > 0;
			if (childHasChildren) nextY += SUBTREE_GAP;
		}

		const y =
			placedChildYs.length > 0
				? (placedChildYs[0] + placedChildYs[placedChildYs.length - 1]) / 2
				: topY;
		positions.set(nodeId, { x: nodeX(node), y });
		visited.add(nodeId);
		visiting.delete(nodeId);
		return Math.max(nextY, y + NODE_Y_STEP);
	}

	let cursorY = CANVAS_PADDING;
	for (const root of orderedRoots) {
		const beforeRootY = cursorY;
		cursorY = layoutNode(root.id, cursorY, new Set());
		const hasChildren = (primaryChildren.get(root.id) ?? []).length > 0;
		const groupGap = hasChildren ? ROOT_GROUP_GAP : STANDALONE_GROUP_GAP;
		if (cursorY > beforeRootY) cursorY += groupGap;
	}

	for (const node of graph.nodes.sort(compareNodes)) {
		if (visited.has(node.id)) continue;
		cursorY = layoutNode(node.id, cursorY, new Set()) + ROOT_GROUP_GAP;
	}

	return positions;
}
