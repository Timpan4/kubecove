<script lang="ts">
	import { cn } from "@/lib/utils";
	import type { UiProps } from "./types";

	let { class: className = "", children, ...rest }: UiProps = $props();
</script>

<div
	data-slot="empty-title"
	class={cn("font-heading text-sm font-medium tracking-tight", className)}
	{...rest}
>
	{@render children?.()}
</div>
