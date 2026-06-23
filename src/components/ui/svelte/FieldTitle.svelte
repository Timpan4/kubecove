<script lang="ts">
	import { cn } from "@/lib/utils";
	import type { UiProps } from "./types";

	let { class: className = "", children, ...rest }: UiProps = $props();
</script>

<div
	data-slot="field-label"
	class={cn(
		"flex w-fit items-center gap-2 text-xs/relaxed font-medium group-data-[disabled=true]/field:opacity-50",
		className,
	)}
	{...rest}
>
	{@render children?.()}
</div>
