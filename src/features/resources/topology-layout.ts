import type { SmoothStepPathOptions } from "@xyflow/system";
import type { TopologyNode } from "@/lib/types";
import { pushMapValue, type TopologyGraph } from "./topology-graph";

export const NODE_WIDTH = 190;
export const NODE_HEIGHT = 78;
const COLUMN_GAP = 104;
const ROW_GAP = 14;
export const CANVAS_PADDING = 96;
const SUBTREE_GAP = 20;
const ROOT_GROUP_GAP = 16;
const STANDALONE_GROUP_GAP = 10;

export interface TopologyLayoutMetrics {
	nodeWidth: number;
	nodeHeight: number;
	columnGap: number;
	rowGap: number;
}

export const DEFAULT_TOPOLOGY_LAYOUT: TopologyLayoutMetrics = {
	nodeWidth: NODE_WIDTH,
	nodeHeight: NODE_HEIGHT,
	columnGap: COLUMN_GAP,
	rowGap: ROW_GAP,
};

export const EDGE_PATH_OPTIONS: SmoothStepPathOptions = {
	borderRadius: 10,
	offset: 22,
	stepPosition: 0.42,
};

interface KindRanks {
	[kind: string]: number;
}

const KIND_RANK: KindRanks = {
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

function compareCreatedAtNewestFirst(aMs: number | null, bMs: number | null): number {
	if (aMs === null && bMs === null) return 0;
	if (aMs === null) return 1;
	if (bMs === null) return -1;
	return bMs - aMs;
}

function createdAtNewestFirstKey(ms: number | null): string {
	if (ms === null) return "1:";
	return `0:${String(Number.MAX_SAFE_INTEGER - ms).padStart(16, "0")}`;
}

// `localeCompare` with an options argument builds a new ICU collator on every
// call. ECMA-402 defines it as `new Intl.Collator(locales, options).compare`,
// so one shared collator keeps the order and skips that cost per comparison.
const NAME_COLLATOR = new Intl.Collator(undefined, { numeric: true });

function sortTopologyNodes(a: TopologyNode, b: TopologyNode, timestamps: ReadonlyMap<string, number | null>): number {
	const rank = kindRank(a.kind) - kindRank(b.kind);
	if (rank !== 0) return rank;
	const namespace = (a.namespace ?? "").localeCompare(b.namespace ?? "");
	if (namespace !== 0) return namespace;
	const kind = a.kind.localeCompare(b.kind);
	if (kind !== 0) return kind;
	const createdAt = compareCreatedAtNewestFirst(timestamps.get(a.id) ?? null, timestamps.get(b.id) ?? null);
	if (createdAt !== 0) return createdAt;
	return NAME_COLLATOR.compare(a.name, b.name);
}

export function topologySortKeys(graph: TopologyGraph) {
	const timestamps = new Map(graph.nodes.map((node) => [node.id, createdAtMs(node)]));
	return {
		compare: (a: TopologyNode, b: TopologyNode) => sortTopologyNodes(a, b, timestamps),
		createdAtKey: (node: TopologyNode) => createdAtNewestFirstKey(timestamps.get(node.id) ?? null),
	};
}

export function topologyColumnDepth({ nodes, nodesById, parents }: TopologyGraph): Map<string, number> {
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
		let edgeDepth = 0;
		for (const parentId of parentIds) {
			const parent = nodesById.get(parentId);
			if (parent) edgeDepth = Math.max(edgeDepth, resolve(parent) + 1);
		}
		const depth = Math.max(edgeDepth, kindRank(node.kind));
		depthById.set(node.id, depth);
		visiting.delete(node.id);
		return depth;
	}

	for (const node of nodes) resolve(node);
	return depthById;
}

export function topologyLayoutOrder(
	{ nodes, nodesById, parents }: TopologyGraph,
	depthById: Map<string, number>,
	sortKeys: ReturnType<typeof topologySortKeys>,
): Map<string, string> {
	const roots = nodes
		.filter((node) => (parents.get(node.id) ?? []).length === 0)
		.sort(sortKeys.compare);
	const rootOrder = new Map(roots.map((node, index) => [node.id, index]));
	const memo = new Map<string, string>();
	const visiting = new Set<string>();
	const localKey = (node: TopologyNode) =>
		[
			String(kindRank(node.kind)).padStart(2, "0"),
			node.namespace ?? "",
			node.kind,
			sortKeys.createdAtKey(node),
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
			.sort(sortKeys.compare);
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
	compare: (a: TopologyNode, b: TopologyNode) => number,
): (a: TopologyNode, b: TopologyNode) => number {
	return (a, b) => {
		const depth = (depthById.get(a.id) ?? 0) - (depthById.get(b.id) ?? 0);
		if (depth !== 0) return depth;
		const order = (orderById.get(a.id) ?? "").localeCompare(
			orderById.get(b.id) ?? "",
		);
		if (order !== 0) return order;
		return compare(a, b);
	};
}

function buildPrimaryChildren(
	graph: TopologyGraph,
	compareNodes: (a: TopologyNode, b: TopologyNode) => number,
): Map<string, string[]> {
	const primaryChildren = new Map<string, string[]>();

	for (const node of graph.nodes) {
		const parentNodes = (graph.parents.get(node.id) ?? [])
			.map((id) => graph.nodesById.get(id))
			.filter((parent): parent is TopologyNode => Boolean(parent))
			.sort(compareNodes);
		const primaryParent = parentNodes[0];
		if (!primaryParent) continue;
		pushMapValue(primaryChildren, primaryParent.id, node.id);
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
	compareNodes: (a: TopologyNode, b: TopologyNode) => number,
	layout: TopologyLayoutMetrics = DEFAULT_TOPOLOGY_LAYOUT,
): Map<string, { x: number; y: number }> {
	const nodeYStep = layout.nodeHeight + layout.rowGap;
	const primaryChildren = buildPrimaryChildren(graph, compareNodes);
	const primaryChildIds = new Set(
		Array.from(primaryChildren.values()).flat(),
	);
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
		roots.length > 0 ? [...ownedRoots, ...standaloneRoots] : [...graph.nodes].sort(compareNodes);
	const positions = new Map<string, { x: number; y: number }>();
	const visited = new Set<string>();

	function nodeX(node: TopologyNode): number {
		const column = depthById.get(node.id) ?? kindRank(node.kind);
		return CANVAS_PADDING + column * (layout.nodeWidth + layout.columnGap);
	}

	function layoutNode(nodeId: string, topY: number, visiting: Set<string>): number {
		const node = graph.nodesById.get(nodeId);
		if (!node || visited.has(nodeId)) return topY;
		if (visiting.has(nodeId)) {
				positions.set(nodeId, { x: nodeX(node), y: topY });
				visited.add(nodeId);
				return topY + nodeYStep;
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
		return Math.max(nextY, y + nodeYStep);
	}

	let cursorY = CANVAS_PADDING;
	for (const root of orderedRoots) {
		const beforeRootY = cursorY;
		cursorY = layoutNode(root.id, cursorY, new Set());
		const hasChildren = (primaryChildren.get(root.id) ?? []).length > 0;
		const groupGap = hasChildren ? ROOT_GROUP_GAP : STANDALONE_GROUP_GAP;
		if (cursorY > beforeRootY) cursorY += groupGap;
	}

	for (const node of [...graph.nodes].sort(compareNodes)) {
		if (visited.has(node.id)) continue;
		cursorY = layoutNode(node.id, cursorY, new Set()) + ROOT_GROUP_GAP;
	}

	return positions;
}
