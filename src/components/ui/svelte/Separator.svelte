<script lang="ts">
	import { cn } from "@/lib/utils";
	import type { UiProps } from "./types";

	let {
		class: className = "",
		orientation = "horizontal",
		decorative = true,
		...rest
	}: UiProps & {
		orientation?: "horizontal" | "vertical";
		decorative?: boolean;
	} = $props();
</script>

<div
	data-slot="separator"
	data-orientation={orientation}
	data-horizontal={orientation === "horizontal" ? "" : undefined}
	data-vertical={orientation === "vertical" ? "" : undefined}
	role={decorative ? "presentation" : "separator"}
	aria-orientation={decorative ? undefined : orientation}
	class={cn(
		"shrink-0 bg-border data-horizontal:h-px data-horizontal:w-full data-vertical:w-px data-vertical:self-stretch",
		className,
	)}
	{...rest}
></div>
