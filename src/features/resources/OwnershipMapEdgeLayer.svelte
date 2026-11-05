<script lang="ts">
	import { MarkerType, useStore, ViewportPortal } from "@xyflow/svelte";
	import { getMarkerId, type EdgeMarker } from "@xyflow/system";
	import { topologyLayerEdgeVisible, type TopologyLayerEdge } from "./topology-edge-layer";

	let { edges }: { edges: TopologyLayerEdge[] } = $props();

	const flow = useStore();
	const markerPrefix = $derived(`${flow.flowId}-edge-layer`);
	const markers = $derived.by(() => {
		const byId = new Map<string, EdgeMarker & { id: string }>();
		for (const edge of edges) {
			if (!edge.markerEnd) continue;
			const id = getMarkerId(edge.markerEnd, markerPrefix);
			if (!byId.has(id)) byId.set(id, { ...edge.markerEnd, id });
		}
		return [...byId.values()];
	});
	// Group visible edges by z so each group gets one stacking layer, in input order like Svelte Flow.
	const groups = $derived.by(() => {
		const view = { viewport: flow.viewport, width: flow.width, height: flow.height };
		const byZ = new Map<number, TopologyLayerEdge[]>();
		for (const edge of edges) {
			if (!topologyLayerEdgeVisible(edge, view)) continue;
			const group = byZ.get(edge.z);
			if (group) group.push(edge);
			else byZ.set(edge.z, [edge]);
		}
		return [...byZ].sort(([a], [b]) => a - b);
	});
</script>

<!-- Svelte Flow draws edges between viewport-back and the node layer, with the same stacking rules. -->
<ViewportPortal target="back">
	<svg class="pointer-events-none absolute size-0 overflow-visible" aria-hidden="true">
		<defs>
			{#each markers as marker (marker.id)}
				<!-- Same markup as Svelte Flow's Marker component. -->
				<marker
					class="svelte-flow__arrowhead"
					id={marker.id}
					markerWidth={`${marker.width ?? 12.5}`}
					markerHeight={`${marker.height ?? 12.5}`}
					viewBox="-10 -10 20 20"
					markerUnits={marker.markerUnits ?? "strokeWidth"}
					orient={marker.orient ?? "auto-start-reverse"}
					refX="0"
					refY="0"
				>
					{#if marker.type === MarkerType.Arrow}
						<polyline
							class="arrow"
							style:stroke={marker.color ?? "none"}
							fill="none"
							stroke-linecap="round"
							stroke-linejoin="round"
							stroke-width={marker.strokeWidth}
							points="-5,-4 0,0 -5,4"
						/>
					{:else if marker.type === MarkerType.ArrowClosed}
						<polyline
							class="arrowclosed"
							style:stroke={marker.color ?? "none"}
							style:fill={marker.color ?? "none"}
							stroke-linecap="round"
							stroke-linejoin="round"
							stroke-width={marker.strokeWidth}
							points="-5,-4 0,0 -5,4 -5,-4"
						/>
					{/if}
				</marker>
			{/each}
		</defs>
	</svg>
	{#each groups as [z, group] (z)}
		<svg class="pointer-events-none absolute overflow-visible" style:z-index={z}>
			{#each group as edge (edge.id)}
				<path
					d={edge.path}
					class="svelte-flow__edge-path"
					marker-end={edge.markerEnd ? `url('#${getMarkerId(edge.markerEnd, markerPrefix)}')` : undefined}
					fill="none"
					style={edge.style}
					role="img"
					aria-roledescription="edge"
					aria-label={`Edge from ${edge.source} to ${edge.target}`}
					data-id={edge.id}
				/>
			{/each}
		</svg>
	{/each}
</ViewportPortal>
