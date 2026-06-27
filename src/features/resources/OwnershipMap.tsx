import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
	Controls,
	ReactFlow,
	useReactFlow,
	type NodeMouseHandler,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import {
	AlertTriangle,
	GitBranch,
	Network,
	PanelLeftClose,
} from "lucide-react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
	Empty,
	EmptyDescription,
	EmptyHeader,
	EmptyTitle,
} from "@/components/ui/empty";
import { Separator } from "@/components/ui/separator";
import { Skeleton } from "@/components/ui/skeleton";
import type {
	ResourceSummary,
	ResourceTopology,
	TopologyMode,
	TopologyNode,
} from "@/lib/types";
import { cnfast } from "@/lib/utils";
import { ownershipMapNodeTypes } from "./OwnershipMapNodes";
import {
	type OwnershipGraphNode,
	type OwnershipResourceGraphNode,
	type StandaloneKindGroupGraphNode,
	buildReactFlowTopologyLayout,
	buildReactFlowTopologySelectionIndex,
	applyReactFlowTopologySelectionWithIndex,
	filterReactFlowTopologyToSelectedRoot,
	type OwnershipGraphEdge,
} from "./topology";
import {
	getOwnershipGraphBounds,
	getOwnershipMapTranslateExtent,
	getOwnershipGraphBoundsForNodeIds,
	ownershipGraphLayoutSignature,
	type OwnershipGraphBounds,
} from "./topology-viewport";

interface OwnershipMapProps {
	topology: ResourceTopology | undefined;
	isLoading: boolean;
	isError: boolean;
	error: unknown;
	selectedNodeId: string | null;
	showFullTopologyOnSelection?: boolean;
	fitViewKey: string;
	mode: TopologyMode;
	heightClassName?: string;
	onModeChange: (mode: TopologyMode) => void;
	onMapToggle: () => void;
	onNodeSelect: (node: TopologyNode, resource: ResourceSummary | null) => void;
}

function errorMessage(error: unknown): string {
	if (error instanceof Error) return error.message;
	if (typeof error === "string") return error;
	if (
		typeof error === "object" &&
		error !== null &&
		"message" in error &&
		typeof error.message === "string"
	) {
		return error.message;
	}
	return "Failed to load ownership map";
}

function OwnershipMapHeader({
	mode,
	nodeCount,
	edgeCount,
	warningCount,
	onModeChange,
	onMapToggle,
}: {
	mode: TopologyMode;
	nodeCount?: number;
	edgeCount?: number;
	warningCount?: number;
	onModeChange: (mode: TopologyMode) => void;
	onMapToggle: () => void;
}) {
	const HeaderIcon = mode === "networkFlow" ? Network : GitBranch;
	const title = mode === "networkFlow" ? "Network Flow" : "Ownership Map";
	return (
		<div className="flex items-center justify-between gap-2 px-3 py-2">
			<div className="flex min-w-0 items-center gap-2">
				<Button
					type="button"
					variant="ghost"
					size="icon"
					className="size-7"
					onClick={onMapToggle}
					aria-label="Collapse ownership map"
					aria-pressed={true}
				>
					<PanelLeftClose />
				</Button>
				<HeaderIcon className="size-4 text-primary" />
				<div className="min-w-0">
					<div className="text-xs font-semibold text-foreground">{title}</div>
					{nodeCount !== undefined && edgeCount !== undefined ? (
						<div className="text-[0.6875rem] text-muted-foreground">
							{nodeCount} nodes · {edgeCount} edges
						</div>
					) : null}
				</div>
			</div>
			<div className="flex shrink-0 items-center gap-2">
				<div className="inline-flex h-8 overflow-hidden rounded-md border bg-background p-0.5">
					<Button
						type="button"
						variant={mode === "ownership" ? "secondary" : "ghost"}
						size="sm"
						className="h-7 rounded-sm px-2 text-xs"
						onClick={() => onModeChange("ownership")}
						aria-pressed={mode === "ownership"}
					>
						<GitBranch data-icon="inline-start" />
						Ownership
					</Button>
					<Button
						type="button"
						variant={mode === "networkFlow" ? "secondary" : "ghost"}
						size="sm"
						className="h-7 rounded-sm px-2 text-xs"
						onClick={() => onModeChange("networkFlow")}
						aria-pressed={mode === "networkFlow"}
					>
						<Network data-icon="inline-start" />
						Network Flow
					</Button>
				</div>
				{warningCount ? (
					<Badge variant="outline" className="rounded-sm">
						<AlertTriangle data-icon="inline-start" />
						{warningCount}
					</Badge>
				) : null}
			</div>
		</div>
	);
}

const WIDTH_FIT_PADDING = 0.08;
const WIDTH_FIT_DURATION_MS = 260;
const MIN_MAP_ZOOM = 0.18;
const MAX_MAP_ZOOM = 1.4;
const REACT_FLOW_PRO_OPTIONS = { hideAttribution: true };

function clampMapZoom(zoom: number): number {
	return Math.min(MAX_MAP_ZOOM, Math.max(MIN_MAP_ZOOM, zoom));
}

function widthFitViewport(
	bounds: OwnershipGraphBounds,
	viewportSize: { width: number; height: number },
) {
	const usableWidth = Math.max(
		1,
		viewportSize.width * (1 - WIDTH_FIT_PADDING * 2),
	);
	const zoom = clampMapZoom(usableWidth / Math.max(1, bounds.width));
	const centerX = bounds.left + bounds.width / 2;
	const centerY = bounds.top + bounds.height / 2;

	return {
		x: viewportSize.width / 2 - centerX * zoom,
		y: viewportSize.height / 2 - centerY * zoom,
		zoom,
	};
}

function isOwnershipResourceNode(
	node: OwnershipGraphNode,
): node is OwnershipResourceGraphNode {
	return node.type === "ownershipResource";
}

function isStandaloneKindGroupNode(
	node: OwnershipGraphNode,
): node is StandaloneKindGroupGraphNode {
	return node.type === "standaloneKindGroup";
}

function selectedStandaloneExpansionId(
	topology: ResourceTopology | undefined,
	selectedNodeId: string | null,
): string | null {
	if (!topology || !selectedNodeId) return null;
	const selectedNode = topology.nodes.find((node) => node.id === selectedNodeId);
	if (!selectedNode) return null;
	const hasRelation = topology.edges.some(
		(edge) => edge.source === selectedNodeId || edge.target === selectedNodeId,
	);
	return hasRelation ? null : selectedNodeId;
}

function selectedOwnershipNodeIds(nodes: OwnershipGraphNode[]): Set<string> {
	const selectedNodeIds = new Set<string>();
	for (const node of nodes) {
		if (
			node.type === "ownershipResource" &&
			(node.data.selected || node.data.connected)
		) {
			selectedNodeIds.add(node.id);
		}
	}
	return selectedNodeIds;
}

function FitOwnershipMapView({
	nodes,
	edges,
	selectedNodeId,
	viewportSize,
	viewportKey,
}: {
	nodes: OwnershipGraphNode[];
	edges: OwnershipGraphEdge[];
	selectedNodeId: string | null;
	viewportSize: { width: number; height: number };
	viewportKey: string;
}) {
	const { setViewport } = useReactFlow();
	const lastViewportRequestRef = useRef<string | null>(null);

	useEffect(() => {
		if (viewportSize.width <= 0 || viewportSize.height <= 0) return;
		const selectedNodeIds = selectedOwnershipNodeIds(nodes);
		const selectedBounds = selectedNodeId
			? getOwnershipGraphBoundsForNodeIds(nodes, selectedNodeIds)
			: null;
		const bounds = selectedBounds ?? getOwnershipGraphBounds(nodes);
		if (!bounds) return;
		const layoutSignature = ownershipGraphLayoutSignature(nodes, edges);
		const requestKey = selectedBounds
			? [
					"selected",
					selectedNodeId,
					viewportKey,
					Math.round(selectedBounds.left),
					Math.round(selectedBounds.top),
					Math.round(selectedBounds.width),
					Math.round(selectedBounds.height),
					Array.from(selectedNodeIds).sort().join(","),
				].join(":")
			: ["all", viewportKey, layoutSignature].join(":");

		if (lastViewportRequestRef.current === requestKey) return;
		lastViewportRequestRef.current = requestKey;

		let secondFrame: number | null = null;
		const firstFrame = window.requestAnimationFrame(() => {
			secondFrame = window.requestAnimationFrame(() => {
				void setViewport(widthFitViewport(bounds, viewportSize), {
					duration: WIDTH_FIT_DURATION_MS,
				});
			});
		});
		return () => {
			window.cancelAnimationFrame(firstFrame);
			if (secondFrame !== null) window.cancelAnimationFrame(secondFrame);
		};
	}, [edges, nodes, selectedNodeId, setViewport, viewportKey, viewportSize]);

	return null;
}

export function OwnershipMap({
	topology,
	isLoading,
	isError,
	error,
	selectedNodeId,
	showFullTopologyOnSelection = false,
	fitViewKey,
	mode,
	heightClassName = "h-[620px]",
	onModeChange,
	onMapToggle,
	onNodeSelect,
}: OwnershipMapProps) {
	const [expandedStandaloneKinds, setExpandedStandaloneKinds] = useState<Set<string>>(
		() => new Set(),
	);
	const standaloneExpansionId = useMemo(
		() => selectedStandaloneExpansionId(topology, selectedNodeId),
		[topology, selectedNodeId],
	);
	const graphLayout = useMemo(
		() =>
			topology
				? buildReactFlowTopologyLayout(topology, standaloneExpansionId, {
					expandedStandaloneKinds,
					groupStandalone: mode === "ownership",
					showPortHints: mode === "networkFlow",
				})
				: null,
		[topology, standaloneExpansionId, expandedStandaloneKinds, mode],
	);
	const selectionIndex = useMemo(
		() => (topology ? buildReactFlowTopologySelectionIndex(topology) : null),
		[topology],
	);
	const graph = useMemo(
		() =>
			graphLayout && selectionIndex
				? applyReactFlowTopologySelectionWithIndex(
					showFullTopologyOnSelection
						? graphLayout
						: filterReactFlowTopologyToSelectedRoot(
								graphLayout,
								selectionIndex,
								selectedNodeId,
							),
					selectionIndex,
					selectedNodeId,
				)
				: null,
		[graphLayout, selectionIndex, selectedNodeId, showFullTopologyOnSelection],
	);
	const [viewportSize, setViewportSize] = useState({ width: 0, height: 0 });
	const resizeObserverRef = useRef<ResizeObserver | null>(null);
	const translateExtent = useMemo(
		() =>
			graph && viewportSize.width > 0 && viewportSize.height > 0
				? getOwnershipMapTranslateExtent(graph.nodes, viewportSize)
				: undefined,
		[graph, viewportSize],
	);
	const viewportSizeKey = `${viewportSize.width}x${viewportSize.height}`;
	const fitViewportKey = `${fitViewKey}:${heightClassName}:${viewportSizeKey}`;
	const setMapViewportRef = useCallback((element: HTMLDivElement | null) => {
		resizeObserverRef.current?.disconnect();
		resizeObserverRef.current = null;
		if (!element) return;
		const updateViewportSize = () => {
			const rect = element.getBoundingClientRect();
			const nextSize = {
				width: Math.round(rect.width),
				height: Math.round(rect.height),
			};
			setViewportSize((currentSize) =>
				currentSize.width === nextSize.width &&
				currentSize.height === nextSize.height
					? currentSize
					: nextSize,
			);
		};
		updateViewportSize();
		const observer = new ResizeObserver(updateViewportSize);
		observer.observe(element);
		resizeObserverRef.current = observer;
	}, []);
	const handleNodeClick = useCallback<NodeMouseHandler<OwnershipGraphNode>>(
		(_, node) => {
			if (isStandaloneKindGroupNode(node)) {
				setExpandedStandaloneKinds((current) => {
					const next = new Set(current);
					if (next.has(node.data.kind)) {
						next.delete(node.data.kind);
					} else {
						next.add(node.data.kind);
					}
					return next;
				});
				return;
			}
			if (!isOwnershipResourceNode(node)) return;
			onNodeSelect(node.data.node, node.data.resource);
		},
		[onNodeSelect],
	);

	useEffect(
		() => () => {
			resizeObserverRef.current?.disconnect();
		},
		[],
	);

	if (isLoading) {
		return (
			<div className="flex h-full min-h-0 flex-col rounded-md border bg-card/60">
				<OwnershipMapHeader
					mode={mode}
					onModeChange={onModeChange}
					onMapToggle={onMapToggle}
				/>
				<Separator />
				<div className="grid min-h-0 flex-1 grid-cols-3 gap-10 p-8">
					{Array.from({ length: 9 }).map((_, index) => (
						<Skeleton key={index} className="h-16 w-48" />
					))}
				</div>
			</div>
		);
	}

	if (isError) {
		return (
			<div className="flex h-full min-h-0 flex-col rounded-md border bg-card/60">
				<OwnershipMapHeader
					mode={mode}
					onModeChange={onModeChange}
					onMapToggle={onMapToggle}
				/>
				<Separator />
				<div className="p-3">
					<Alert variant="destructive">
						<AlertTitle>
							{mode === "networkFlow"
								? "Failed to load network flow"
								: "Failed to load ownership map"}
						</AlertTitle>
						<AlertDescription>{errorMessage(error)}</AlertDescription>
					</Alert>
				</div>
			</div>
		);
	}

	if (!topology || topology.nodes.length === 0 || !graph) {
		return (
			<div className="flex h-full min-h-0 flex-col rounded-md border bg-card/60">
				<OwnershipMapHeader
					mode={mode}
					nodeCount={topology?.nodes.length}
					edgeCount={0}
					onModeChange={onModeChange}
					onMapToggle={onMapToggle}
				/>
				<Separator />
				<Empty className="min-h-52 flex-1 border-0">
					<EmptyHeader>
						<EmptyTitle>
							{mode === "networkFlow" ? "No network flow" : "No ownership graph"}
						</EmptyTitle>
						<EmptyDescription>
							{mode === "networkFlow"
								? "No ingress, service, or pod traffic relationships were found in this scope."
								: "No workload ownership relationships were found in this scope."}
						</EmptyDescription>
					</EmptyHeader>
				</Empty>
			</div>
		);
	}

	return (
		<div className="flex h-full min-h-0 flex-col overflow-hidden rounded-md border bg-card/60">
			<OwnershipMapHeader
				mode={mode}
				nodeCount={topology.nodes.length}
				edgeCount={graph.edges.length}
				warningCount={topology.warnings.length}
				onModeChange={onModeChange}
				onMapToggle={onMapToggle}
			/>
			<Separator />
			<div
				ref={setMapViewportRef}
				className={cnfast(heightClassName, "flex-1 bg-background")}
			>
				<ReactFlow
					nodes={graph.nodes}
					edges={graph.edges}
					nodeTypes={ownershipMapNodeTypes}
					onNodeClick={handleNodeClick}
					translateExtent={translateExtent}
					minZoom={MIN_MAP_ZOOM}
					maxZoom={MAX_MAP_ZOOM}
					nodesDraggable={false}
					nodesConnectable={false}
					edgesFocusable={false}
					connectOnClick={false}
					zoomOnDoubleClick={false}
					deleteKeyCode={null}
					proOptions={REACT_FLOW_PRO_OPTIONS}
					className="ownership-map-flow"
				>
					<FitOwnershipMapView
						nodes={graph.nodes}
						edges={graph.edges}
						selectedNodeId={selectedNodeId}
						viewportSize={viewportSize}
						viewportKey={fitViewportKey}
					/>
					<Controls
						position="top-left"
						orientation="horizontal"
						showInteractive={false}
					/>
				</ReactFlow>
			</div>
			{topology.warnings.length > 0 && (
				<>
					<Separator />
					<div className="flex flex-col gap-1 px-3 py-2 text-[0.6875rem] text-muted-foreground">
						<div className="font-semibold text-foreground">
							Topology warnings
						</div>
						{topology.warnings.slice(0, 3).map((warning) => (
							<div key={warning} className="truncate">
								{warning}
							</div>
						))}
						{topology.warnings.length > 3 && (
							<div>{topology.warnings.length - 3} more warnings</div>
						)}
					</div>
				</>
			)}
		</div>
	);
}
