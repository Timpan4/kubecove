<script lang="ts">
	import { Command as CommandPrimitive } from "bits-ui";
	import { cnfast } from "@/lib/utils";
	import type { UiProps } from "./types";

	let { class: className = "", children, ...rest }: UiProps = $props();
</script>

<CommandPrimitive.Root
	data-slot="command"
	class={cnfast(
		"flex h-full w-full flex-col overflow-hidden rounded-md bg-popover text-popover-foreground",
		className,
	)}
	{...rest}
>
	{@render children?.()}
</CommandPrimitive.Root>
