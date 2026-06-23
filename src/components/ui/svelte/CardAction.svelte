<script lang="ts">
	import { cn } from "@/lib/utils";
	import type { UiProps } from "./types";

	let { class: className = "", children, ...rest }: UiProps = $props();
</script>

<div
	data-slot="card-action"
	class={cn(
		"col-start-2 row-span-2 row-start-1 self-start justify-self-end",
		className,
	)}
	{...rest}
>
	{@render children?.()}
</div>
