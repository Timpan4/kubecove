<script lang="ts">
	import { cnfast } from "@/lib/utils";
	import type { UiProps } from "./types";

	let { class: className = "", children, ...rest }: UiProps = $props();
</script>

<div
	data-slot="empty-content"
	class={cnfast(
		"flex w-full max-w-sm min-w-0 flex-col items-center gap-2 text-xs/relaxed text-balance",
		className,
	)}
	{...rest}
>
	{@render children?.()}
</div>
