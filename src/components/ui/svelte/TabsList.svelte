<script lang="ts">
	import { Tabs as TabsPrimitive } from "bits-ui";
	import { cn } from "@/lib/utils";
	import { tabsListVariants } from "./classes";
	import type { UiProps } from "./types";

	let {
		class: className = "",
		variant = "default",
		children,
		...rest
	}: UiProps & { variant?: "default" | "line" } = $props();
</script>

<TabsPrimitive.List
	data-slot="tabs-list"
	data-variant={variant}
	class={cn(tabsListVariants({ variant }), className)}
	{...rest}
>
	{@render children?.()}
</TabsPrimitive.List>
