<script lang="ts">
	import { cnfast } from "@/lib/utils";
	import { badgeVariants, type BadgeVariant } from "./classes";
	import type { UiProps } from "./types";

	let {
		class: className = "",
		variant = "default",
		children,
		...rest
	}: UiProps & { variant?: BadgeVariant } = $props();
</script>

<span
	data-slot="badge"
	data-variant={variant}
	class={cnfast(badgeVariants({ variant }), className)}
	{...rest}
>
	{@render children?.()}
</span>
