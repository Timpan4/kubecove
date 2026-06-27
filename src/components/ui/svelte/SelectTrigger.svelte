<script lang="ts">
	import { Select as SelectPrimitive } from "bits-ui";
	import { ChevronDownIcon } from "lucide-svelte";
	import { cnfast } from "@/lib/utils";
	import type { UiProps } from "./types";

	let {
		class: className = "",
		size = "default",
		children,
		...rest
	}: UiProps & { size?: "sm" | "default" } = $props();
</script>

<SelectPrimitive.Trigger
	data-slot="select-trigger"
	data-size={size}
	class={cnfast(
		"flex w-fit cursor-pointer items-center justify-between gap-1.5 rounded-md border border-input bg-input/20 px-2 py-1.5 text-xs/relaxed whitespace-nowrap transition-colors outline-none focus-visible:border-ring focus-visible:ring-2 focus-visible:ring-ring/30 disabled:pointer-events-none disabled:cursor-not-allowed disabled:opacity-50 aria-invalid:border-destructive aria-invalid:ring-2 aria-invalid:ring-destructive/20 data-placeholder:text-muted-foreground data-[size=default]:h-7 data-[size=sm]:h-6 *:data-[slot=select-value]:line-clamp-1 *:data-[slot=select-value]:flex *:data-[slot=select-value]:items-center *:data-[slot=select-value]:gap-1.5 dark:bg-input/30 dark:hover:bg-input/50 dark:aria-invalid:border-destructive/50 dark:aria-invalid:ring-destructive/40 [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-3.5",
		className,
	)}
	{...rest}
>
	{@render children?.()}
	<ChevronDownIcon class="pointer-events-none size-3.5 text-muted-foreground" />
</SelectPrimitive.Trigger>
