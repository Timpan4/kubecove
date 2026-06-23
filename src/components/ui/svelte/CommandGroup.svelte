<script lang="ts">
	import { Command as CommandPrimitive } from "bits-ui";
	import { cn } from "@/lib/utils";
	import type { UiProps } from "./types";

	let { class: className = "", children, ...rest }: UiProps = $props();
</script>

<CommandPrimitive.Group
	data-slot="command-group"
	class={cn(
		"overflow-hidden py-1 text-foreground [&_[cmdk-group-heading]]:py-1.5 [&_[cmdk-group-heading]]:text-[10px] [&_[cmdk-group-heading]]:uppercase [&_[cmdk-group-heading]]:tracking-wide",
		className,
	)}
	{...rest}
>
	{@render children?.()}
</CommandPrimitive.Group>
