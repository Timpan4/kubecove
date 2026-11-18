<script lang="ts">
	import { setContext } from "svelte";
	import { prefersReducedMotion } from "svelte/motion";
	import { RESOURCE_ACTIVITY_CURRENT } from "./resource-activity-animation";
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
	import { loadingDelay } from "@/lib/loading-delay.svelte";
	import type { ResourceSummary, ResourceTopology, TopologyMode } from "@/lib/types";
	import {
		buildFlowTopologyLayoutState,
		buildFlowTopologyViewFromLayoutState,
		selectedStandaloneExpansionId as resolveSelectedStandaloneExpansionId,
		type FlowTopologyNode,
	} from "./topology";
	import { buildFlowTopologyEdgeLayer, buildFlowTopologyPulses } from "./topology-edge-layer";
	import MapContextSelect from "./MapContextSelect.svelte";
	import OwnershipMapEdgeLayer from "./OwnershipMapEdgeLayer.svelte";
	import OwnershipMapPulseLayer from "./OwnershipMapPulseLayer.svelte";
	import OwnershipMapViewport from "./OwnershipMapViewport.svelte";
	import OwnershipResourceNode from "./OwnershipResourceNode.svelte";
	import StandaloneKindGroupNode from "./StandaloneKindGroupNode.svelte";

	const nodeTypes: NodeTypes = {
		ownershipResource: OwnershipResourceNode,
		standaloneKindGroup: StandaloneKindGroupNode,
	};
	// Keep small graphs mounted so Tab can reach offscreen nodes and pan them into view.
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
		updatesCurrent = true,
		contexts = [],
		mapContext = "",
		onMapContextChange = () => {},
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
		updatesCurrent?: boolean;
		/** Workspace contexts in rank order; more than one shows the Map context select. */
		contexts?: readonly string[];
		mapContext?: string;
		onMapContextChange?: (context: string) => void;
	} = $props();
	setContext(RESOURCE_ACTIVITY_CURRENT, () => updatesCurrent);
	const loading = loadingDelay(() => isLoading);

	let viewportWidth = $state(0);
	let viewportHeight = $state(0);
	let afterGraphElement = $state<HTMLDivElement>();
	let mapHeading = $state<HTMLDivElement>();
	let refreshErrorDetails = $state<HTMLDetailsElement>();
	let errorBranch: "graph" | "empty" | null = null;

	$effect.pre(() => {
		const next = error && topology && !loading.active && !isError
			? (topology.nodes.length && graph ? "graph" : "empty")
			: null;
		if (next !== errorBranch && refreshErrorDetails?.contains(document.activeElement)) {
			mapHeading?.focus({ preventScroll: true });
		}
		errorBranch = next;
	});
	let expandedStandaloneKinds = $state<Set<string>>(new Set());
	const hasViewportSize = $derived(viewportWidth > 0 && viewportHeight > 0);
	const selectedStandaloneExpansionId = $derived(
		topology ? resolveSelectedStandaloneExpansionId(topology, selectedNodeId) : null,
	);
	const topologyLayoutState = $derived(
		topology
			? buildFlowTopologyLayoutState(
					topology,
					mode,
					expandedStandaloneKinds,
					selectedStandaloneExpansionId,
				)
			: null,
	);
	const topologyView = $derived(
		topologyLayoutState
			? buildFlowTopologyViewFromLayoutState(topologyLayoutState, {
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
	// Culled maps draw their edges in one SVG layer; see topology-edge-layer.ts.
	const edgeLayer = $derived(
		graph && onlyRenderVisibleElements
			? buildFlowTopologyEdgeLayer(graph.nodes, graph.edges)
			: null,
	);
	// Continuous edge motion repaints every frame: about a tenth of a core in WebKitGTK on a small
	// map. So it runs only on maps small enough to stay fully mounted, and only in the active window.
	let windowActive = $state(document.hasFocus());
	const edgeMotion = $derived(
		!onlyRenderVisibleElements && !prefersReducedMotion.current && windowActive,
	);
	// The pulse stops with the activity icons when live updates are not current.
	const pulses = $derived(
		graph && edgeMotion && updatesCurrent ? buildFlowTopologyPulses(graph.nodes, graph.edges) : [],
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

<svelte:window onfocus={() => (windowActive = true)} onblur={() => (windowActive = false)} />

<div class="relative flex h-full min-h-[400px] min-w-0 flex-1 flex-col overflow-hidden rounded-lg border bg-surface-1 shadow-sm">
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
				<div class="truncate text-xs font-semibold" bind:this={mapHeading} tabindex="-1">{title}</div>
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
			{#if contexts.length > 1}
				<MapContextSelect {contexts} value={mapContext} onChange={onMapContextChange} />
			{/if}
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

	{#if loading.active}
		<div class="flex min-h-80 flex-1 items-center justify-center gap-2 text-xs text-muted-foreground">
			{#if loading.shown}
				<Spinner />
				<span>Loading topology</span>
			{/if}
		</div>
	{:else if isError}
		<div class="p-3">
			<FriendlyError
				{error}
				context={{ operation: "resourcesLoad", fallbackTitle: errorTitle }}
			/>
		</div>
	{:else if !topology || topology.nodes.length === 0 || !graph}
		<div class="relative flex min-h-0 flex-1">
			<Empty class="flex-1 border-0">
				<EmptyHeader>
					<EmptyTitle>{emptyTitle}</EmptyTitle>
					<EmptyDescription>{emptyDescription}</EmptyDescription>
				</EmptyHeader>
			</Empty>
			{#if topology && error}{@render mapRefreshError()}{/if}
		</div>
	{:else}
		<p class="border-b px-3 py-1.5 text-[0.6875rem] text-muted-foreground">
			Drag to pan. Scroll or use controls to zoom. Press Tab to reach graph nodes, or
			<Button
				variant="link"
				size="xs"
				class="h-auto px-0 align-baseline"
				onclick={() => afterGraphElement?.focus()}
			>skip graph</Button>.
		</p>
		<div
			class="relative min-h-0 flex-1"
			bind:clientWidth={viewportWidth}
			bind:clientHeight={viewportHeight}
		>
			{#if hasViewportSize}
				<SvelteFlow
					nodes={graph.nodes}
					edges={edgeLayer?.native ?? graph.edges}
					{nodeTypes}
					{translateExtent}
					class={edgeMotion ? "ownership-map-flow ownership-map-motion" : "ownership-map-flow"}
					minZoom={0.12}
					maxZoom={1.4}
					nodesDraggable={false}
					nodesConnectable={false}
					panOnDrag={true}
					panOnScroll={false}
					preventScrolling={true}
					edgesFocusable={false}
					zoomOnDoubleClick={false}
					zoomOnScroll={true}
					{onlyRenderVisibleElements}
					proOptions={{ hideAttribution: true }}
					onnodeclick={handleNodeClick}
				>
					{#if edgeLayer}
						<OwnershipMapEdgeLayer edges={edgeLayer.layered} />
					{/if}
					{#if pulses.length > 0}
						<OwnershipMapPulseLayer {pulses} />
					{/if}
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
			<div class="absolute inset-x-0 bottom-0" bind:this={afterGraphElement} tabindex="-1"></div>
			{#if error}{@render mapRefreshError()}{/if}
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

{#snippet mapRefreshError()}
	<details bind:this={refreshErrorDetails} class="absolute right-3 bottom-3 z-20 max-h-[calc(100%_-_1.5rem)] max-w-[calc(100%_-_1.5rem)] overflow-auto rounded-md border bg-surface-1 px-3 py-2 text-xs text-muted-foreground shadow-lg">
		<summary class="cursor-pointer">Map refresh failed</summary>
		<FriendlyError {error} context={{ operation: "resourcesLoad", fallbackTitle: errorTitle }} />
	</details>
{/snippet}
