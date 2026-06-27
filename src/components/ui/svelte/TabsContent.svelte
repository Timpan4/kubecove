<script lang="ts">
	import { Tabs as TabsPrimitive } from "bits-ui";
	import { cnfast } from "@/lib/utils";
	import type { UiProps } from "./types";

	let {
		class: className = "",
		value,
		children,
		...rest
	}: UiProps & { value: string } = $props();
</script>

<TabsPrimitive.Content
	data-slot="tabs-content"
	{value}
	class={cnfast("flex-1 text-xs/relaxed outline-none", className)}
	{...rest}
>
	{@render children?.()}
</TabsPrimitive.Content>
