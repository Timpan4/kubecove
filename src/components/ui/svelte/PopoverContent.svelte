<script lang="ts">
	import { Popover as PopoverPrimitive } from "bits-ui";
	import { cn } from "@/lib/utils";
	import type { UiProps } from "./types";

	let {
		class: className = "",
		children,
		align = "center",
		sideOffset = 4,
		...rest
	}: UiProps & { align?: "start" | "center" | "end"; sideOffset?: number } = $props();
</script>

<PopoverPrimitive.Portal>
	<PopoverPrimitive.Content
		data-slot="popover-content"
		class={cn(
			"z-popover w-72 rounded-md border bg-surface-2 p-4 text-popover-foreground shadow-xl outline-none data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=closed]:zoom-out-95 data-[state=open]:animate-in data-[state=open]:fade-in-0 data-[state=open]:zoom-in-95",
			className,
		)}
		{align}
		{sideOffset}
		{...rest}
	>
		{@render children?.()}
	</PopoverPrimitive.Content>
</PopoverPrimitive.Portal>
