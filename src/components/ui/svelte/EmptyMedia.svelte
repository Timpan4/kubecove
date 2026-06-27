<script lang="ts">
	import { cnfast } from "@/lib/utils";
	import { emptyMediaVariants } from "./classes";
	import type { UiProps } from "./types";

	let {
		class: className = "",
		variant = "default",
		children,
		...rest
	}: UiProps & { variant?: "default" | "icon" } = $props();
</script>

<div
	data-slot="empty-icon"
	data-variant={variant}
	class={cnfast(emptyMediaVariants({ variant }), className)}
	{...rest}
>
	{@render children?.()}
</div>
