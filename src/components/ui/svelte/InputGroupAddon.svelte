<script lang="ts">
	import { cn } from "@/lib/utils";
	import type { UiProps } from "./types";

	let {
		class: className = "",
		align = "inline-start",
		children,
		...rest
	}: UiProps & {
		align?: "inline-start" | "inline-end" | "block-start" | "block-end";
	} = $props();
</script>

<div
	role="group"
	data-slot="input-group-addon"
	data-align={align}
	class={cn(
		"flex h-auto cursor-text items-center justify-center gap-1 py-2 text-xs/relaxed font-medium text-muted-foreground select-none group-data-[disabled=true]/input-group:opacity-50 [&>svg:not([class*='size-'])]:size-3.5",
		align === "inline-start" &&
			"order-first pl-2 has-[>button]:ml-[-0.275rem] has-[>kbd]:ml-[-0.275rem]",
		align === "inline-end" &&
			"order-last pr-2 has-[>button]:mr-[-0.275rem] has-[>kbd]:mr-[-0.275rem]",
		align === "block-start" &&
			"order-first w-full justify-start px-2 pt-2 group-has-[>input]/input-group:pt-2 [.border-b]:pb-2",
		align === "block-end" &&
			"order-last w-full justify-start px-2 pb-2 group-has-[>input]/input-group:pb-2 [.border-t]:pt-2",
		className,
	)}
	{...rest}
>
	{@render children?.()}
</div>
