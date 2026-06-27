<script lang="ts">
	import { cnfast } from "@/lib/utils";
	import type { UiProps } from "./types";

	let { class: className = "", children, ...rest }: UiProps = $props();
</script>

<div
	data-slot="empty-header"
	class={cnfast("flex max-w-sm flex-col items-center gap-1", className)}
	{...rest}
>
	{@render children?.()}
</div>
