<script lang="ts">
	import { cn } from "@/lib/utils";
	import { fieldVariants, type FieldOrientation } from "./classes";
	import type { UiProps } from "./types";

	let {
		class: className = "",
		orientation = "vertical",
		children,
		...rest
	}: UiProps & { orientation?: FieldOrientation } = $props();
</script>

<div
	role="group"
	data-slot="field"
	data-orientation={orientation}
	class={cn(fieldVariants({ orientation }), className)}
	{...rest}
>
	{@render children?.()}
</div>
