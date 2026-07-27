import type { NodeBase } from "@xyflow/system";
import type { TopologyNode } from "@/lib/types";
import { CANVAS_PADDING, NODE_HEIGHT, kindRank } from "./topology-layout";
import type { TopologyGraph } from "./topology-graph";

export interface StandaloneKindGroupNodeData extends Record<string, unknown> {
	kind: string;
	count: number;
	nodeIds: string[];
	dimmed: boolean;
	expanded: boolean;
}

export type StandaloneKindGroupGraphNode = NodeBase<
	StandaloneKindGroupNodeData,
	"standaloneKindGroup"
> & {
	style?: Record<string, number>;
};

export interface StandaloneGroupLayoutOptions {
	nodeHeight?: number;
	standaloneNodeWidth?: number;
}

const STANDALONE_GROUP_COLUMNS = 3;
export const STANDALONE_NODE_WIDTH = 260;
const STANDALONE_GROUP_HEADER_HEIGHT = 38;
const STANDALONE_COLLAPSED_HEIGHT = 42;
const STANDALONE_GROUP_PADDING = 14;
const STANDALONE_GROUP_COLUMN_GAP = 14;
const STANDALONE_GROUP_ROW_GAP = 10;
const STANDALONE_GROUP_GAP = 24;

function standaloneGroupId(kind: string): string {
	return `standalone-kind:${kind}`;
}

function standaloneGroupWidth(columns: number, standaloneNodeWidth: number): number {
	return (
		STANDALONE_GROUP_PADDING * 2 +
		columns * standaloneNodeWidth +
		Math.max(0, columns - 1) * STANDALONE_GROUP_COLUMN_GAP
	);
}

function standaloneGroupHeight(rows: number, nodeHeight: number): number {
	return (
		STANDALONE_GROUP_HEADER_HEIGHT +
		STANDALONE_GROUP_PADDING * 2 +
		rows * nodeHeight +
		Math.max(0, rows - 1) * STANDALONE_GROUP_ROW_GAP
	);
}

function standaloneCollapsedGroupWidth(count: number): number {
	return Math.min(360, Math.max(190, 156 + String(count).length * 8));
}

export function buildStandaloneGroups(
	graph: TopologyGraph,
	positions: Map<string, { x: number; y: number }>,
	compareNodes: (a: TopologyNode, b: TopologyNode) => number,
	selectedPathNodeIds: Set<string>,
	hasSelection: boolean,
	expandedStandaloneKinds: ReadonlySet<string>,
	selectedNodeId: string | null,
	layout: StandaloneGroupLayoutOptions = {},
): {
	groupNodes: StandaloneKindGroupGraphNode[];
	positionsById: Map<string, { x: number; y: number }>;
	standaloneIds: Set<string>;
	groupIdByNodeId: Map<string, string>;
} {
	const nodeHeight = layout.nodeHeight ?? NODE_HEIGHT;
	const standaloneNodeWidth = layout.standaloneNodeWidth ?? STANDALONE_NODE_WIDTH;
	const standaloneNodes = graph.nodes
		.filter(
			(node) =>
				(graph.parents.get(node.id) ?? []).length === 0 &&
				(graph.children.get(node.id) ?? []).length === 0,
		)
		.sort(compareNodes);
	const standaloneIds = new Set(standaloneNodes.map((node) => node.id));
	const ownedPositions = graph.nodes
		.filter((node) => !standaloneIds.has(node.id))
		.map((node) => positions.get(node.id))
		.filter((position): position is { x: number; y: number } => Boolean(position));
	const ownedBottom =
		ownedPositions.length > 0
			? Math.max(...ownedPositions.map((position) => position.y + nodeHeight))
			: CANVAS_PADDING - STANDALONE_GROUP_GAP;
	const groupedByKind = new Map<string, TopologyNode[]>();
	for (const node of standaloneNodes) {
		const bucket = groupedByKind.get(node.kind);
		if (bucket) {
			bucket.push(node);
		} else {
			groupedByKind.set(node.kind, [node]);
		}
	}

	const groupNodes: StandaloneKindGroupGraphNode[] = [];
	const positionsById = new Map<string, { x: number; y: number }>();
	const groupIdByNodeId = new Map<string, string>();
	let cursorY = ownedBottom + STANDALONE_GROUP_GAP;
	for (const [kind, nodes] of Array.from(groupedByKind.entries()).sort(
		([a], [b]) => {
			const rank = kindRank(a) - kindRank(b);
			return rank !== 0 ? rank : a.localeCompare(b);
		},
	)) {
		const expanded =
			expandedStandaloneKinds.has(kind) ||
			nodes.some((node) => node.id === selectedNodeId);
		const columns = Math.min(STANDALONE_GROUP_COLUMNS, Math.max(1, nodes.length));
		const rows = expanded ? Math.ceil(nodes.length / columns) : 0;
		const groupId = standaloneGroupId(kind);
		const width = expanded
			? standaloneGroupWidth(columns, standaloneNodeWidth)
			: standaloneCollapsedGroupWidth(nodes.length);
		const height = expanded
			? standaloneGroupHeight(rows, nodeHeight)
			: STANDALONE_COLLAPSED_HEIGHT;
		const groupDimmed =
			hasSelection && !nodes.some((node) => selectedPathNodeIds.has(node.id));
		groupNodes.push({
			id: groupId,
			type: "standaloneKindGroup",
			position: { x: CANVAS_PADDING, y: cursorY },
			data: {
				kind,
				count: nodes.length,
				nodeIds: nodes.map((node) => node.id),
				dimmed: groupDimmed,
				expanded,
			},
			draggable: false,
			selectable: true,
			connectable: false,
			style: { width, height },
			zIndex: 0,
		});
		if (expanded) {
			nodes.forEach((node, index) => {
				const column = index % columns;
				const row = Math.floor(index / columns);
				groupIdByNodeId.set(node.id, groupId);
				positionsById.set(node.id, {
					x:
						STANDALONE_GROUP_PADDING +
						column * (standaloneNodeWidth + STANDALONE_GROUP_COLUMN_GAP),
					y:
						STANDALONE_GROUP_HEADER_HEIGHT +
						STANDALONE_GROUP_PADDING +
						row * (nodeHeight + STANDALONE_GROUP_ROW_GAP),
				});
			});
		}
		cursorY += height + STANDALONE_GROUP_GAP;
	}

	return { groupNodes, positionsById, standaloneIds, groupIdByNodeId };
}
