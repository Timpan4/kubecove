<script lang="ts">
	import { cnfast } from "@/lib/utils";
	import { alertVariants } from "./classes";
	import type { UiProps } from "./types";

	let {
		class: className = "",
		variant = "default",
		children,
		...rest
	}: UiProps & { variant?: "default" | "destructive" } = $props();
</script>

<div
	data-slot="alert"
	role="alert"
	class={cnfast(alertVariants({ variant }), className)}
	{...rest}
>
	{@render children?.()}
</div>
