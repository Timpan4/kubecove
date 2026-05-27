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
	buildReactFlowTopology,
	type OwnershipGraphNode,
	type OwnershipResourceGraphNode,
	type StandaloneKindGroupGraphNode,
} from "./topology";
import {
	absoluteGraphNodePosition,
	getOwnershipMapTranslateExtent,
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
const NODE_CENTER_OFFSET = { x: 95, y: 39 };
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

function FitTopologyView({ fitViewKey }: { fitViewKey: string }) {
	const { fitView } = useReactFlow();
	const lastFitViewKeyRef = useRef<string | null>(null);

	useEffect(() => {
		if (lastFitViewKeyRef.current === fitViewKey) return;
		lastFitViewKeyRef.current = fitViewKey;
		const frame = window.requestAnimationFrame(() => {
			void fitView(FIT_VIEW_OPTIONS);
		});
		return () => window.cancelAnimationFrame(frame);
	}, [fitView, fitViewKey]);

	return null;
}

function CenterSelectedNode({
	nodes,
	selectedNodeId,
	viewportKey,
}: {
	nodes: OwnershipGraphNode[];
	selectedNodeId: string | null;
	viewportKey: string;
}) {
	const { getZoom, setCenter } = useReactFlow();
	const lastCenterRequestRef = useRef<string | null>(null);

	useEffect(() => {
		if (!selectedNodeId) return;
		const selectedNode = nodes.find((node) => node.id === selectedNodeId);
		if (!selectedNode) return;
		const selectedPosition = absoluteGraphNodePosition(nodes, selectedNode);
		const requestKey = [
			selectedNodeId,
			viewportKey,
			Math.round(selectedPosition.x),
			Math.round(selectedPosition.y),
		].join(":");
		if (lastCenterRequestRef.current === requestKey) return;
		lastCenterRequestRef.current = requestKey;
		let secondFrame: number | null = null;
		const firstFrame = window.requestAnimationFrame(() => {
			secondFrame = window.requestAnimationFrame(() => {
				void setCenter(
					selectedPosition.x + NODE_CENTER_OFFSET.x,
					selectedPosition.y + NODE_CENTER_OFFSET.y,
					{ duration: 260, zoom: getZoom() },
				);
			});
		});
		return () => {
			window.cancelAnimationFrame(firstFrame);
			if (secondFrame !== null) window.cancelAnimationFrame(secondFrame);
		};
	}, [getZoom, nodes, selectedNodeId, setCenter, viewportKey]);

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
	const mapViewportElementRef = useRef<HTMLDivElement | null>(null);
	const graph = useMemo(
		() =>
			topology
				? buildReactFlowTopology(topology, selectedNodeId, {
					expandedStandaloneKinds,
					groupStandalone: mode === "ownership",
					showPortHints: mode === "networkFlow",
				})
				: null,
		[topology, selectedNodeId, expandedStandaloneKinds, mode],
	);
	const [viewportSize, setViewportSize] = useState({ width: 0, height: 0 });
	const translateExtent = useMemo(
		() =>
			graph && viewportSize.width > 0 && viewportSize.height > 0
				? getOwnershipMapTranslateExtent(graph.nodes, viewportSize)
				: undefined,
		[graph, viewportSize],
	);
	const viewportSizeKey = `${viewportSize.width}x${viewportSize.height}`;
	const centerViewportKey = `${heightClassName}:${viewportSizeKey}`;
	const setMapViewportRef = useCallback((element: HTMLDivElement | null) => {
		mapViewportElementRef.current = element;
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

	useEffect(() => {
		const mapViewportElement = mapViewportElementRef.current;
		if (!mapViewportElement) return;
		const updateViewportSize = () => {
			const rect = mapViewportElement.getBoundingClientRect();
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
		observer.observe(mapViewportElement);
		return () => observer.disconnect();
	}, []);

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
					<FitTopologyView fitViewKey={fitViewKey} />
					<CenterSelectedNode
						nodes={graph.nodes}
						selectedNodeId={selectedNodeId}
						viewportKey={centerViewportKey}
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
