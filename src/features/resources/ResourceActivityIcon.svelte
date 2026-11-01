<script lang="ts">
	import { drawResourceActivity } from "./resource-activity-animation";
	let { active = true }: { active?: boolean } = $props();
	let canvas = $state<HTMLCanvasElement>();
	$effect(() => canvas ? drawResourceActivity(canvas, active) : undefined);
</script>

<canvas bind:this={canvas} class="size-3 shrink-0" aria-hidden="true" data-resource-progress={active ? "active" : "paused"}></canvas>
