<script lang="ts">
	import { Command as CommandPrimitive } from "bits-ui";
	import { cnfast } from "@/lib/utils";
	import type { UiProps } from "./types";

	let { class: className = "", children, ...rest }: UiProps = $props();
</script>

<CommandPrimitive.Item
	data-slot="command-item"
	class={cnfast(
		"relative flex cursor-default select-none items-center gap-2 rounded-sm text-xs outline-none data-[disabled=true]:pointer-events-none data-[selected=true]:bg-accent data-[selected=true]:text-accent-foreground data-[disabled=true]:opacity-50",
		className,
	)}
	{...rest}
>
	{@render children?.()}
</CommandPrimitive.Item>
