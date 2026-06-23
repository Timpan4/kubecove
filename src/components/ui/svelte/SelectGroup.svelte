<script lang="ts">
	import { Select as SelectPrimitive } from "bits-ui";
	import { cn } from "@/lib/utils";
	import type { UiProps } from "./types";

	let { class: className = "", children, ...rest }: UiProps = $props();
</script>

<SelectPrimitive.Group
	data-slot="select-group"
	class={cn("scroll-my-1 p-1", className)}
	{...rest}
>
	{@render children?.()}
</SelectPrimitive.Group>
