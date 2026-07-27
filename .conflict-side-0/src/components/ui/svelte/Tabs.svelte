<script lang="ts">
	import { Tabs as TabsPrimitive } from "bits-ui";
	import { cnfast } from "@/lib/utils";
	import type { UiProps } from "./types";

	let {
		class: className = "",
		orientation = "horizontal",
		children,
		...rest
	}: UiProps & { orientation?: "horizontal" | "vertical" } = $props();
</script>

<TabsPrimitive.Root
	data-slot="tabs"
	data-orientation={orientation}
	class={cnfast("group/tabs flex gap-2 data-[orientation=horizontal]:flex-col", className)}
	{orientation}
	{...rest}
>
	{@render children?.()}
</TabsPrimitive.Root>
