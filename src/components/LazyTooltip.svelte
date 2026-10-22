<script lang="ts">
	import { tick, type Snippet } from "svelte";
	import type { HTMLButtonAttributes } from "svelte/elements";
	import Tooltip from "./ui/svelte/Tooltip.svelte";
	import TooltipContent from "./ui/svelte/TooltipContent.svelte";
	import TooltipTrigger from "./ui/svelte/TooltipTrigger.svelte";

	let {
		children,
		content,
		contentClass = "",
		side = "top",
		align = "center",
		sideOffset = 8,
		...triggerProps
	}: Omit<HTMLButtonAttributes, "class"> & {
		class?: string;
		children: Snippet;
		content: Snippet;
		contentClass?: string;
		side?: "top" | "right" | "bottom" | "left";
		align?: "start" | "center" | "end";
		sideOffset?: number;
	} = $props();

	const triggerId = $props.id();
	let ready = $state(false);

	async function activate(focused: boolean) {
		ready = true;
		await tick();
		if (focused) document.getElementById(triggerId)?.focus({ preventScroll: true });
	}
</script>

{#if ready}
	<Tooltip>
		<TooltipTrigger type="button" {...triggerProps} id={triggerId}>
			{@render children()}
		</TooltipTrigger>
		<TooltipContent {side} {align} {sideOffset} class={contentClass}>
			{@render content()}
		</TooltipContent>
	</Tooltip>
{:else}
	<button type="button" {...triggerProps} id={triggerId}
		onpointerenter={() => void activate(false)} onfocus={() => void activate(true)}>
		{@render children()}
	</button>
{/if}
