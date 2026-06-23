<script lang="ts">
	import { cn } from "@/lib/utils";
	import type { UiProps } from "./types";

	let { class: className = "", children, ...rest }: UiProps = $props();
</script>

<div
	data-slot="empty"
	class={cn(
		"flex w-full min-w-0 flex-1 flex-col items-center justify-center gap-4 rounded-xl border-dashed p-6 text-center text-balance",
		className,
	)}
	{...rest}
>
	{@render children?.()}
</div>
