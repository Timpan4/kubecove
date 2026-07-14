<script lang="ts">
	import { Controls, SvelteFlow, type NodeTypes } from "@xyflow/svelte";
	import type { NodeEventWithPointer } from "@xyflow/svelte";
	import "@xyflow/svelte/dist/style.css";
	import { GitBranch, Network, PanelLeftClose } from "lucide-svelte";
	import FriendlyError from "@/components/FriendlyError.svelte";
	import {
		Badge,
		Button,
		Empty,
		EmptyDescription,
		EmptyHeader,
		EmptyTitle,
		Spinner,
	} from "@/components/ui/svelte";
	import type { ResourceSummary, ResourceTopology, TopologyMode } from "@/lib/types";
	import {
		buildFlowTopologyView,
		type FlowTopologyNode,
	} from "./topology";
	import OwnershipMapViewport from "./OwnershipMapViewport.svelte";
	import OwnershipResourceNode from "./OwnershipResourceNode.svelte";
	import StandaloneKindGroupNode from "./StandaloneKindGroupNode.svelte";

	const nodeTypes: NodeTypes = {
		ownershipResource: OwnershipResourceNode,
		standaloneKindGroup: StandaloneKindGroupNode,
	};
	// ponytail: tune threshold from real cluster profiles if this still flickers or lags.
	const VISIBLE_ELEMENT_RENDER_THRESHOLD = 120;

	let {
		topology,
		isLoading,
		isError,
		error,
		mode,
		selectedNodeId,
		showFullTopologyOnSelection = false,
		fitViewKey,
		onModeChange,
		onNodeSelect,
		onMapToggle,
	}: {
		topology: ResourceTopology | undefined;
		isLoading: boolean;
		isError: boolean;
		error: unknown;
		mode: TopologyMode;
		selectedNodeId: string | null;
		showFullTopologyOnSelection?: boolean;
		fitViewKey: string;
		onModeChange: (mode: TopologyMode) => void;
		onNodeSelect: (nodeId: string, resource: ResourceSummary | null) => void;
		onMapToggle?: () => void;
	} = $props();

	let viewportWidth = $state(0);
	let viewportHeight = $state(0);
	let expandedStandaloneKinds = $state<Set<string>>(new Set());
	const hasViewportSize = $derived(viewportWidth > 0 && viewportHeight > 0);
	const topologyView = $derived(
		topology
			? buildFlowTopologyView(topology, {
					mode,
					selectedNodeId,
					showFullTopologyOnSelection,
					expandedStandaloneKinds,
					viewportSize: hasViewportSize
						? { width: viewportWidth, height: viewportHeight }
						: undefined,
				})
			: null,
	);
	const graph = $derived(topologyView?.graph ?? null);
	const translateExtent = $derived(topologyView?.translateExtent);
	const onlyRenderVisibleElements = $derived(
		graph
			? graph.nodes.length + graph.edges.length >= VISIBLE_ELEMENT_RENDER_THRESHOLD
			: false,
	);
	const focusedGraph = $derived(
		Boolean(
			selectedNodeId &&
				!showFullTopologyOnSelection &&
				graph &&
				topology &&
				graph.nodes.length < topology.nodes.length,
		),
	);
	const fitViewportKey = $derived(`${fitViewKey}:${viewportWidth}x${viewportHeight}`);
	const title = $derived(mode === "networkFlow" ? "Network Flow" : "Ownership Map");
	const errorTitle = $derived(
		mode === "networkFlow" ? "Failed to load network flow" : "Failed to load ownership map",
	);
	const emptyTitle = $derived(mode === "networkFlow" ? "No network flow" : "No ownership graph");
	const emptyDescription = $derived(
		mode === "networkFlow"
			? "No ingress, service, or pod traffic relationships were found in this scope."
			: "No workload ownership relationships were found in this scope.",
	);

	const handleNodeClick: NodeEventWithPointer<MouseEvent | TouchEvent, FlowTopologyNode> = ({
		node,
	}) => {
		if (node.id.startsWith("standalone-kind:") && node.data.kind) {
			const next = new Set(expandedStandaloneKinds);
			if (next.has(node.data.kind)) {
				next.delete(node.data.kind);
			} else {
				next.add(node.data.kind);
			}
			expandedStandaloneKinds = next;
			return;
		}
		onNodeSelect(node.id, node.data.resource);
	};

</script>

<div class="flex h-full min-h-[400px] min-w-0 flex-1 flex-col overflow-hidden rounded-lg border bg-surface-1 shadow-sm">
	<div class="flex flex-wrap items-center justify-between gap-2 border-b px-3 py-2">
		<div class="flex min-w-0 items-center gap-2">
			{#if onMapToggle}
				<Button
					type="button"
					variant="ghost"
					size="icon"
					class="size-7"
					onclick={onMapToggle}
					aria-label="Collapse ownership map"
					aria-pressed={true}
				>
					<PanelLeftClose />
				</Button>
			{/if}
			{#if mode === "networkFlow"}
				<Network />
			{:else}
				<GitBranch />
			{/if}
			<div class="min-w-0">
				<div class="truncate text-xs font-semibold">{title}</div>
				<div class="text-[0.6875rem] text-muted-foreground">
					{#if focusedGraph}
						{graph?.nodes.length ?? 0} focused / {topology?.nodes.length ?? 0} nodes
						/ {graph?.edges.length ?? 0} edges
					{:else}
						{topology?.nodes.length ?? 0} nodes / {graph?.edges.length ?? 0} edges
					{/if}
				</div>
			</div>
		</div>
		<div class="flex items-center gap-2">
			<div class="inline-flex h-8 overflow-hidden rounded-md border bg-background p-0.5">
				<Button
					variant={mode === "ownership" ? "secondary" : "ghost"}
					size="sm"
					class="h-7 rounded-sm px-2 text-xs"
					onclick={() => onModeChange("ownership")}
					aria-pressed={mode === "ownership"}
				>
					<GitBranch data-icon="inline-start" />
					Ownership
				</Button>
				<Button
					variant={mode === "networkFlow" ? "secondary" : "ghost"}
					size="sm"
					class="h-7 rounded-sm px-2 text-xs"
					onclick={() => onModeChange("networkFlow")}
					aria-pressed={mode === "networkFlow"}
				>
					<Network data-icon="inline-start" />
					Network Flow
				</Button>
			</div>
			{#if topology?.warnings.length}
				<Badge variant="outline">{topology.warnings.length}</Badge>
			{/if}
		</div>
	</div>

	{#if isLoading}
		<div class="flex min-h-80 flex-1 items-center justify-center gap-2 text-xs text-muted-foreground">
			<Spinner />
			<span>Loading topology</span>
		</div>
	{:else if isError}
		<div class="p-3">
			<FriendlyError
				{error}
				context={{ operation: "resourcesLoad", fallbackTitle: errorTitle }}
			/>
		</div>
	{:else if !topology || topology.nodes.length === 0 || !graph}
		<Empty class="min-h-80 flex-1 border-0">
			<EmptyHeader>
				<EmptyTitle>{emptyTitle}</EmptyTitle>
				<EmptyDescription>{emptyDescription}</EmptyDescription>
			</EmptyHeader>
		</Empty>
	{:else}
		<div
			class="min-h-80 flex-1"
			bind:clientWidth={viewportWidth}
			bind:clientHeight={viewportHeight}
		>
			{#if hasViewportSize}
				<SvelteFlow
					nodes={graph.nodes}
					edges={graph.edges}
					{nodeTypes}
					{translateExtent}
					class="ownership-map-flow"
					minZoom={0.12}
					maxZoom={1.4}
					nodesDraggable={false}
					nodesConnectable={false}
					panOnDrag={true}
					edgesFocusable={false}
					zoomOnDoubleClick={false}
					{onlyRenderVisibleElements}
					proOptions={{ hideAttribution: true }}
					onnodeclick={handleNodeClick}
				>
					<OwnershipMapViewport
						nodes={graph.nodes}
						edges={graph.edges}
						{selectedNodeId}
						{viewportWidth}
						{viewportHeight}
						viewportKey={fitViewportKey}
					/>
					<Controls position="top-left" orientation="horizontal" showLock={false} />
				</SvelteFlow>
			{/if}
		</div>
		{#if topology.warnings.length > 0}
			<div class="flex flex-col gap-1 border-t px-3 py-2 text-[0.6875rem] text-muted-foreground">
				<div class="font-semibold text-foreground">Topology warnings</div>
				{#each topology.warnings.slice(0, 3) as warning (warning)}
					<div class="truncate">{warning}</div>
				{/each}
				{#if topology.warnings.length > 3}
					<div>{topology.warnings.length - 3} more warnings</div>
				{/if}
			</div>
		{/if}
	{/if}
</div>
