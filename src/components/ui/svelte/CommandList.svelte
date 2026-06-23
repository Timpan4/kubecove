<script lang="ts">
	import { Command as CommandPrimitive } from "bits-ui";
	import { cn } from "@/lib/utils";
	import type { UiProps } from "./types";

	let { class: className = "", children, ...rest }: UiProps = $props();
</script>

<CommandPrimitive.List
	data-slot="command-list"
	class={cn("max-h-[320px] scroll-py-1 overflow-y-auto overflow-x-hidden", className)}
	{...rest}
>
	{@render children?.()}
</CommandPrimitive.List>
