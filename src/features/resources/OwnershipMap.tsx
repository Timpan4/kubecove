import { useEffect, useMemo, useRef, useState } from "react";
import {
	Background,
	BackgroundVariant,
	Controls,
	Handle,
	Position,
	ReactFlow,
	useReactFlow,
	type NodeMouseHandler,
	type NodeProps,
	type NodeTypes,
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
import { getResourceKindVisual } from "@/lib/resource-visuals";
import type { ResourceSummary, ResourceTopology, TopologyNode } from "@/lib/types";
import { cn } from "@/lib/utils";
import {
	buildReactFlowTopology,
	topologyNodeClassName,
	type OwnershipGraphNode,
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

function HealthBadge({ health }: { health: string }) {
	return (
		<Badge
			variant={health === "degraded" ? "destructive" : "outline"}
			className="h-4 rounded-sm px-1.5 text-[0.625rem]"
		>
			{health}
		</Badge>
	);
}

function OwnershipResourceNode({
	data,
	selected,
}: NodeProps<OwnershipGraphNode>) {
	const node = data.node;
	const visual = getResourceKindVisual(node.kind);
	const Icon = visual.icon;
	const selectedOrConnected = data.selected || data.connected;

	return (
		<div
			className={cn(
				topologyNodeClassName(
					node,
					data.selected || selected ? node.id : null,
					node.id,
					selectedOrConnected,
				),
				"relative flex h-[66px] w-[190px] flex-col justify-between px-2.5 py-2",
				data.dimmed && "opacity-35",
			)}
			data-selected={data.selected ? "true" : undefined}
		>
			<Handle
				type="target"
				position={Position.Left}
				isConnectable={false}
				style={{ opacity: 0 }}
			/>
			<Handle
				type="source"
				position={Position.Right}
				isConnectable={false}
				style={{ opacity: 0 }}
			/>
			<div className="flex min-w-0 items-start justify-between gap-2">
				<div className="flex min-w-0 items-start gap-2">
					<Icon className={cn("mt-0.5 size-3.5 shrink-0", visual.className)} />
					<div className="min-w-0">
						<div className="truncate text-[0.6875rem] font-semibold text-foreground">
							{node.name}
						</div>
						<div className="mt-0.5 flex min-w-0 items-center gap-1.5 text-[0.625rem] text-muted-foreground">
							<span className="truncate">{node.kind}</span>
							{node.namespace && <span className="truncate">{node.namespace}</span>}
						</div>
					</div>
				</div>
				<HealthBadge health={node.health} />
			</div>
			{node.status && (
				<div className="truncate text-[0.625rem] text-muted-foreground">
					{node.status}
				</div>
			)}
		</div>
	);
}

const nodeTypes = {
	ownershipResource: OwnershipResourceNode,
} satisfies NodeTypes;

const FIT_VIEW_OPTIONS = { padding: 0.24, maxZoom: 1 };
const NODE_CENTER_OFFSET = { x: 95, y: 33 };

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
		const requestKey = `${selectedNodeId}:${viewportKey}`;
		if (lastCenterRequestRef.current === requestKey) return;
		lastCenterRequestRef.current = requestKey;
		let secondFrame: number | null = null;
		const firstFrame = window.requestAnimationFrame(() => {
			secondFrame = window.requestAnimationFrame(() => {
				void setCenter(
					selectedNode.position.x + NODE_CENTER_OFFSET.x,
					selectedNode.position.y + NODE_CENTER_OFFSET.y,
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
	const graph = useMemo(
		() => (topology ? buildReactFlowTopology(topology, selectedNodeId) : null),
		[topology, selectedNodeId],
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
							{graph.nodes.length} nodes · {graph.edges.length} edges
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
					nodeTypes={nodeTypes}
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
