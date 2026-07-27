<script lang="ts">
	import { cnfast } from "@/lib/utils";
	import { cardVariants, type CardElevation } from "./classes";
	import type { UiProps } from "./types";

	let {
		class: className = "",
		size = "default",
		elevation = "flat",
		children,
		...rest
	}: UiProps & { size?: "default" | "sm"; elevation?: CardElevation } = $props();
</script>

<div
	data-slot="card"
	data-size={size}
	class={cnfast(cardVariants({ elevation }), className)}
	{...rest}
>
	{@render children?.()}
</div>
