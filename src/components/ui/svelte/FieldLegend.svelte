<script lang="ts">
	import { cn } from "@/lib/utils";
	import type { UiProps } from "./types";

	let {
		class: className = "",
		variant = "legend",
		children,
		...rest
	}: UiProps & { variant?: "legend" | "label" } = $props();
</script>

<legend
	data-slot="field-legend"
	data-variant={variant}
	class={cn(
		"mb-2 font-medium data-[variant=label]:text-xs/relaxed data-[variant=legend]:text-sm",
		className,
	)}
	{...rest}
>
	{@render children?.()}
</legend>
