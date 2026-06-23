<script lang="ts">
	import { Select as SelectPrimitive } from "bits-ui";
	import { ChevronUpIcon } from "lucide-svelte";
	import { cn } from "@/lib/utils";
	import type { UiProps } from "./types";

	let { class: className = "", children, ...rest }: UiProps = $props();
</script>

<SelectPrimitive.ScrollUpButton
	data-slot="select-scroll-up-button"
	class={cn(
		"z-10 flex cursor-default items-center justify-center bg-popover py-1 [&_svg:not([class*='size-'])]:size-3.5",
		className,
	)}
	{...rest}
>
	{#if children}
		{@render children()}
	{:else}
		<ChevronUpIcon />
	{/if}
</SelectPrimitive.ScrollUpButton>
