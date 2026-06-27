<script lang="ts">
	import { Select as SelectPrimitive } from "bits-ui";
	import { CheckIcon } from "lucide-svelte";
	import { cnfast } from "@/lib/utils";
	import type { UiProps } from "./types";

	let {
		class: className = "",
		children: itemChildren,
		value,
		label,
		...rest
	}: UiProps & { value: string; label?: string } = $props();
</script>

<SelectPrimitive.Item
	data-slot="select-item"
	class={cnfast(
		"relative flex min-h-7 w-full cursor-default items-center gap-2 rounded-md py-1 pr-7 pl-2 text-xs/relaxed outline-hidden select-none focus:bg-accent focus:text-accent-foreground not-data-[variant=destructive]:focus:**:text-accent-foreground data-disabled:pointer-events-none data-disabled:opacity-50 [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-3.5 *:[span]:last:flex *:[span]:last:min-w-0 *:[span]:last:items-center *:[span]:last:gap-2 *:[span]:last:truncate",
		className,
	)}
	{value}
	{label}
	{...rest}
>
	{#snippet children({ selected })}
		<span class="pointer-events-none absolute right-2 flex items-center justify-center">
			{#if selected}
				<CheckIcon />
			{/if}
		</span>
		<span>
			{@render itemChildren?.()}
		</span>
	{/snippet}
</SelectPrimitive.Item>
