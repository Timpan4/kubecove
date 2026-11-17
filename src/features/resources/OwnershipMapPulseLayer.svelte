<script lang="ts">
	import { ViewportPortal } from "@xyflow/svelte";
	import type { TopologyEdgePulse } from "./topology-edge-layer";

	let { pulses }: { pulses: TopologyEdgePulse[] } = $props();
</script>

<!--
	Each pulse is a dot that App.css moves along its edge. It is drawn above the nodes, outside
	every node card subtree, and fades out before it reaches a card.
-->
<ViewportPortal target="front">
	<!-- A zero-size svg would not render: its paths draw through the overflow, as in the edge layer. -->
	<svg class="pointer-events-none absolute overflow-visible" aria-hidden="true">
		{#each pulses as pulse (pulse.id)}
			<g class="stroke-sky-700 dark:stroke-sky-300" data-pulse={pulse.id}>
				<path d={pulse.path} pathLength="1" class="ownership-map-pulse ownership-map-pulse-halo" />
				<path d={pulse.path} pathLength="1" class="ownership-map-pulse" />
			</g>
		{/each}
	</svg>
</ViewportPortal>
