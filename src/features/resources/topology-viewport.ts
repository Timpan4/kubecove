import type { CoordinateExtent } from "@xyflow/react";
import { NODE_HEIGHT, NODE_WIDTH } from "./topology-layout";
import type { OwnershipGraphEdge, OwnershipGraphNode } from "./topology";

export interface OwnershipMapViewportSize {
	width: number;
	height: number;
}

export interface OwnershipGraphBounds {
	left: number;
	top: number;
	right: number;
	bottom: number;
	width: number;
	height: number;
}

const PAN_PADDING_RATIO = 0.42;
const MIN_PAN_PADDING = 180;
const MAX_PAN_PADDING = 720;
const ZERO_TRANSLATE_EXTENT: CoordinateExtent = [
	[0, 0],
	[0, 0],
];

function clamp(value: number, min: number, max: number): number {
	return Math.min(max, Math.max(min, value));
}

function finitePositiveNumber(value: unknown): number | null {
	if (typeof value !== "number" || !Number.isFinite(value) || value <= 0) {
		return null;
	}
	return value;
}

function nodeStyleDimension(
	node: OwnershipGraphNode,
	dimension: "width" | "height",
): number | null {
	return finitePositiveNumber(
		(node.style as Record<string, unknown> | undefined)?.[dimension],
	);
}

function nodeMeasuredDimension(
	node: OwnershipGraphNode,
	dimension: "width" | "height",
): number | null {
	const measuredNode = node as {
		width?: number;
		height?: number;
		measured?: { width?: number; height?: number };
	};
	return (
		finitePositiveNumber(measuredNode[dimension]) ??
		finitePositiveNumber(measuredNode.measured?.[dimension])
	);
}

function nodeDimensions(node: OwnershipGraphNode): {
	width: number;
	height: number;
} {
	return {
		width:
			nodeStyleDimension(node, "width") ??
			nodeMeasuredDimension(node, "width") ??
			NODE_WIDTH,
		height:
			nodeStyleDimension(node, "height") ??
			nodeMeasuredDimension(node, "height") ??
			NODE_HEIGHT,
	};
}

function warnInDevelopment(message: string): void {
	const env = (import.meta as { env?: { DEV?: boolean } }).env;
	if (env?.DEV) console.warn(message);
}

export function ownershipMapBoundaryPadding(
	viewportSize: OwnershipMapViewportSize,
): { x: number; y: number } {
	return {
		x: clamp(
			viewportSize.width * PAN_PADDING_RATIO,
			MIN_PAN_PADDING,
			MAX_PAN_PADDING,
		),
		y: clamp(
			viewportSize.height * PAN_PADDING_RATIO,
			MIN_PAN_PADDING,
			MAX_PAN_PADDING,
		),
	};
}

function absoluteGraphNodePositionFromLookup(
	nodesById: ReadonlyMap<string, OwnershipGraphNode>,
	node: OwnershipGraphNode,
): { x: number; y: number } {
	let x = node.position.x;
	let y = node.position.y;
	let parentId = node.parentId;
	const visitedParentIds = new Set<string>();

	while (parentId) {
		if (visitedParentIds.has(parentId)) {
			warnInDevelopment(
				`Cycle detected in ownership map parent chain for node "${node.id}" at parent "${parentId}".`,
			);
			break;
		}
		visitedParentIds.add(parentId);
		const parent = nodesById.get(parentId);
		if (!parent) break;
		x += parent.position.x;
		y += parent.position.y;
		parentId = parent.parentId;
	}

	return { x, y };
}

export function absoluteGraphNodePosition(
	nodes: OwnershipGraphNode[],
	node: OwnershipGraphNode,
): { x: number; y: number } {
	const nodesById = new Map(nodes.map((candidate) => [candidate.id, candidate]));
	return absoluteGraphNodePositionFromLookup(nodesById, node);
}

export function getOwnershipGraphBounds(
	nodes: OwnershipGraphNode[],
): OwnershipGraphBounds | null {
	if (nodes.length === 0) return null;

	const nodesById = new Map(nodes.map((node) => [node.id, node]));
	let left = Number.POSITIVE_INFINITY;
	let top = Number.POSITIVE_INFINITY;
	let right = Number.NEGATIVE_INFINITY;
	let bottom = Number.NEGATIVE_INFINITY;

	for (const node of nodes) {
		const position = absoluteGraphNodePositionFromLookup(nodesById, node);
		const dimensions = nodeDimensions(node);
		left = Math.min(left, position.x);
		top = Math.min(top, position.y);
		right = Math.max(right, position.x + dimensions.width);
		bottom = Math.max(bottom, position.y + dimensions.height);
	}

	if (!Number.isFinite(left + top + right + bottom)) return null;

	return {
		left,
		top,
		right,
		bottom,
		width: right - left,
		height: bottom - top,
	};
}

export function getOwnershipMapTranslateExtent(
	nodes: OwnershipGraphNode[],
	viewportSize: OwnershipMapViewportSize,
): CoordinateExtent {
	if (viewportSize.width <= 0 || viewportSize.height <= 0) {
		return ZERO_TRANSLATE_EXTENT;
	}

	const bounds = getOwnershipGraphBounds(nodes);
	if (!bounds) return ZERO_TRANSLATE_EXTENT;

	const padding = ownershipMapBoundaryPadding(viewportSize);
	return [
		[bounds.left - padding.x, bounds.top - padding.y],
		[bounds.right + padding.x, bounds.bottom + padding.y],
	];
}

export function ownershipGraphLayoutSignature(
	nodes: OwnershipGraphNode[],
	edges: OwnershipGraphEdge[],
): string {
	const nodesById = new Map(nodes.map((node) => [node.id, node]));
	const nodeParts = nodes
		.map((node) => {
			const position = absoluteGraphNodePositionFromLookup(nodesById, node);
			const dimensions = nodeDimensions(node);
			return [
				node.id,
				node.parentId ?? "",
				node.type ?? "",
				position.x,
				position.y,
				dimensions.width,
				dimensions.height,
			].join(":");
		})
		.sort();
	const edgeParts = edges.map((edge) => edge.id).sort();
	return `${nodeParts.join("|")}::${edgeParts.join("|")}`;
}
