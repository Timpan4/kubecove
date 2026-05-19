import { useEffect, useMemo, useRef, useState } from "react";
import {
	Background,
	BackgroundVariant,
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
import type { ResourceSummary, ResourceTopology, TopologyNode } from "@/lib/types";
import { cn } from "@/lib/utils";
import { ownershipMapNodeTypes } from "./OwnershipMapNodes";
import {
	buildReactFlowTopology,
	type OwnershipGraphNode,
	type OwnershipResourceGraphNode,
	type StandaloneKindGroupGraphNode,
} from "./topology";

interface OwnershipMapProps {
	topology: ResourceTopology | undefined;
	isLoading: boolean;
	isError: boolean;
	error: unknown;
	selectedNodeId: string | null;
	heightClassName?: string;
	onNodeSelect: (node: TopologyNode, resource: ResourceSummary | null) => void;
}

function errorMessage(error: unknown): string {
	return error instanceof Error ? error.message : "Failed to load ownership map";
}

const FIT_VIEW_OPTIONS = { padding: 0.24, maxZoom: 1 };
const NODE_CENTER_OFFSET = { x: 95, y: 39 };

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

function absoluteNodePosition(
	nodes: OwnershipGraphNode[],
	node: OwnershipGraphNode,
): { x: number; y: number } {
	let x = node.position.x;
	let y = node.position.y;
	let parentId = node.parentId;
	while (parentId) {
		const parent = nodes.find((candidate) => candidate.id === parentId);
		if (!parent) break;
		x += parent.position.x;
		y += parent.position.y;
		parentId = parent.parentId;
	}
	return { x, y };
}

function FitTopologyView({ signature }: { signature: string }) {
	const { fitView } = useReactFlow();
	const lastFitSignatureRef = useRef<string | null>(null);

	useEffect(() => {
		if (lastFitSignatureRef.current === signature) return;
		lastFitSignatureRef.current = signature;
		const frame = window.requestAnimationFrame(() => {
			void fitView(FIT_VIEW_OPTIONS);
		});
		return () => window.cancelAnimationFrame(frame);
	}, [fitView, signature]);

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
		const selectedPosition = absoluteNodePosition(nodes, selectedNode);
		const requestKey = `${selectedNodeId}:${viewportKey}`;
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
	heightClassName = "h-[620px]",
	onNodeSelect,
}: OwnershipMapProps) {
	const [expandedStandaloneKinds, setExpandedStandaloneKinds] = useState<Set<string>>(
		() => new Set(),
	);
	const graph = useMemo(
		() =>
			topology
				? buildReactFlowTopology(topology, selectedNodeId, {
						expandedStandaloneKinds,
					})
				: null,
		[topology, selectedNodeId, expandedStandaloneKinds],
	);
	const mapViewportRef = useRef<HTMLDivElement>(null);
	const [viewportSizeKey, setViewportSizeKey] = useState("0x0");
	const topologySignature = useMemo(() => {
		if (!topology) return "";
		const nodeIds = topology.nodes.map((node) => node.id).sort().join("|");
		const edgeIds = topology.edges.map((edge) => edge.id).sort().join("|");
		return `${nodeIds}::${edgeIds}`;
	}, [topology]);
	const centerViewportKey = `${heightClassName}:${viewportSizeKey}`;
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
		const element = mapViewportRef.current;
		if (!element) return;
		const updateViewportSize = () => {
			const rect = element.getBoundingClientRect();
			setViewportSizeKey(`${Math.round(rect.width)}x${Math.round(rect.height)}`);
		};
		updateViewportSize();
		const observer = new ResizeObserver(updateViewportSize);
		observer.observe(element);
		return () => observer.disconnect();
	}, []);

	if (isLoading) {
		return (
			<div className="rounded-md border bg-card/60">
				<div className="flex items-center gap-2 px-3 py-2 text-xs font-semibold text-foreground">
					<Network className="size-4" />
					Ownership Map
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
				<AlertTitle>Failed to load ownership map</AlertTitle>
				<AlertDescription>{errorMessage(error)}</AlertDescription>
			</Alert>
		);
	}

	if (!topology || topology.nodes.length === 0 || !graph) {
		return (
			<Empty className="min-h-52 border">
				<EmptyHeader>
					<EmptyTitle>No ownership graph</EmptyTitle>
					<EmptyDescription>
						No workload ownership relationships were found in this scope.
					</EmptyDescription>
				</EmptyHeader>
			</Empty>
		);
	}

	return (
		<div className="overflow-hidden rounded-md border bg-card/60">
			<div className="flex items-center justify-between gap-2 px-3 py-2">
				<div className="flex min-w-0 items-center gap-2">
					<GitBranch className="size-4 text-primary" />
					<div className="min-w-0">
						<div className="text-xs font-semibold text-foreground">
							Ownership Map
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
			<div ref={mapViewportRef} className={cn(heightClassName, "bg-background")}>
				<ReactFlow
					nodes={graph.nodes}
					edges={graph.edges}
					nodeTypes={ownershipMapNodeTypes}
					onNodeClick={handleNodeClick}
					minZoom={0.18}
					maxZoom={1.4}
					nodesDraggable={false}
					nodesConnectable={false}
					edgesFocusable={false}
					connectOnClick={false}
					zoomOnDoubleClick={false}
					deleteKeyCode={null}
					proOptions={{ hideAttribution: true }}
					className="[&_.react-flow__attribution]:bg-background/80 [&_.react-flow__controls-button]:border-border [&_.react-flow__controls-button]:bg-card [&_.react-flow__controls-button]:text-foreground [&_.react-flow__controls-button]:shadow-none [&_.react-flow__controls-button:hover]:bg-accent [&_.react-flow__controls-button_svg]:fill-current [&_.react-flow__controls-button_svg]:stroke-current [&_.react-flow__controls]:overflow-hidden [&_.react-flow__controls]:rounded-md [&_.react-flow__controls]:border [&_.react-flow__controls]:border-border [&_.react-flow__controls]:bg-card [&_.react-flow__edge-path]:stroke-muted-foreground/55"
				>
					<FitTopologyView signature={topologySignature} />
					<CenterSelectedNode
						nodes={graph.nodes}
						selectedNodeId={selectedNodeId}
						viewportKey={centerViewportKey}
					/>
					<Background
						variant={BackgroundVariant.Dots}
						gap={22}
						size={1}
						className="opacity-45"
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
					<div className="truncate px-3 py-2 text-[0.6875rem] text-muted-foreground">
						{topology.warnings[0]}
					</div>
				</>
			)}
		</div>
	);
}
