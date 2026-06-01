import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
	Controls,
	ReactFlow,
	useReactFlow,
	type NodeMouseHandler,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import { AlertTriangle, GitBranch, Network } from "lucide-react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
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
import { cn } from "@/lib/utils";
import { ownershipMapNodeTypes } from "./OwnershipMapNodes";
import {
	type OwnershipGraphNode,
	type OwnershipResourceGraphNode,
	type StandaloneKindGroupGraphNode,
	buildReactFlowTopologyLayout,
	buildReactFlowTopologySelectionIndex,
	applyReactFlowTopologySelectionWithIndex,
	type OwnershipGraphEdge,
} from "./topology";
import {
	getOwnershipMapTranslateExtent,
	getOwnershipGraphBoundsForNodeIds,
	ownershipGraphBoundsToViewportRect,
	ownershipGraphLayoutSignature,
} from "./topology-viewport";

interface OwnershipMapProps {
	topology: ResourceTopology | undefined;
	isLoading: boolean;
	isError: boolean;
	error: unknown;
	selectedNodeId: string | null;
	fitViewKey: string;
	mode: TopologyMode;
	heightClassName?: string;
	onNodeSelect: (node: TopologyNode, resource: ResourceSummary | null) => void;
}

function errorMessage(error: unknown): string {
	return error instanceof Error ? error.message : "Failed to load ownership map";
}

const FIT_VIEW_OPTIONS = { padding: 0.24, maxZoom: 1 };
const FIT_SELECTED_BOUNDS_OPTIONS = { padding: 0.28, duration: 260 };
const MIN_MAP_ZOOM = 0.18;
const MAX_MAP_ZOOM = 1.4;

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
	viewportKey,
}: {
	nodes: OwnershipGraphNode[];
	edges: OwnershipGraphEdge[];
	selectedNodeId: string | null;
	viewportKey: string;
}) {
	const { fitBounds, fitView } = useReactFlow();
	const lastViewportRequestRef = useRef<string | null>(null);

	useEffect(() => {
		const selectedNodeIds = selectedOwnershipNodeIds(nodes);
		const selectedBounds = selectedNodeId
			? getOwnershipGraphBoundsForNodeIds(nodes, selectedNodeIds)
			: null;
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
				if (selectedBounds) {
					void fitBounds(
						ownershipGraphBoundsToViewportRect(selectedBounds),
						FIT_SELECTED_BOUNDS_OPTIONS,
					);
				} else {
					void fitView(FIT_VIEW_OPTIONS);
				}
			});
		});
		return () => {
			window.cancelAnimationFrame(firstFrame);
			if (secondFrame !== null) window.cancelAnimationFrame(secondFrame);
		};
	}, [edges, fitBounds, fitView, nodes, selectedNodeId, viewportKey]);

	return null;
}

export function OwnershipMap({
	topology,
	isLoading,
	isError,
	error,
	selectedNodeId,
	fitViewKey,
	mode,
	heightClassName = "h-[620px]",
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
					graphLayout,
					selectionIndex,
					selectedNodeId,
				)
				: null,
		[graphLayout, selectionIndex, selectedNodeId],
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
	const handleNodeClick: NodeMouseHandler<OwnershipGraphNode> = (_, node) => {
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
	};

	useEffect(
		() => () => {
			resizeObserverRef.current?.disconnect();
		},
		[],
	);

	if (isLoading) {
		const HeaderIcon = mode === "networkFlow" ? Network : GitBranch;
		return (
			<div className="rounded-md border bg-card/60">
				<div className="flex items-center gap-2 px-3 py-2 text-xs font-semibold text-foreground">
					<HeaderIcon className="size-4" />
					{mode === "networkFlow" ? "Network Flow" : "Ownership Map"}
				</div>
				<Separator />
				<div className="grid h-[520px] grid-cols-3 gap-10 p-8">
					{Array.from({ length: 9 }).map((_, index) => (
						<Skeleton key={index} className="h-16 w-48" />
					))}
				</div>
			</div>
		);
	}

	if (isError) {
		return (
			<Alert variant="destructive">
				<AlertTitle>
					{mode === "networkFlow"
						? "Failed to load network flow"
						: "Failed to load ownership map"}
				</AlertTitle>
				<AlertDescription>{errorMessage(error)}</AlertDescription>
			</Alert>
		);
	}

	if (!topology || topology.nodes.length === 0 || !graph) {
		return (
			<Empty className="min-h-52 border">
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
		);
	}

	return (
		<div className="overflow-hidden rounded-md border bg-card/60">
			<div className="flex items-center justify-between gap-2 px-3 py-2">
				<div className="flex min-w-0 items-center gap-2">
					{mode === "networkFlow" ? (
						<Network className="size-4 text-primary" />
					) : (
						<GitBranch className="size-4 text-primary" />
					)}
					<div className="min-w-0">
						<div className="text-xs font-semibold text-foreground">
							{mode === "networkFlow" ? "Network Flow" : "Ownership Map"}
						</div>
						<div className="text-[0.6875rem] text-muted-foreground">
							{topology.nodes.length} nodes · {graph.edges.length} edges
						</div>
					</div>
				</div>
				{topology.warnings.length > 0 && (
					<Badge variant="outline" className="rounded-sm">
						<AlertTriangle data-icon="inline-start" />
						{topology.warnings.length}
					</Badge>
				)}
			</div>
			<Separator />
			<div ref={setMapViewportRef} className={cn(heightClassName, "bg-background")}>
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
					proOptions={{ hideAttribution: true }}
					className="ownership-map-flow"
				>
					<FitOwnershipMapView
						nodes={graph.nodes}
						edges={graph.edges}
						selectedNodeId={selectedNodeId}
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
