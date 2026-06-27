<script lang="ts">
	import { cnfast } from "@/lib/utils";
	import type { UiProps } from "./types";

	let { class: className = "", children, ...rest }: UiProps = $props();
</script>

<div
	data-slot="card-title"
	class={cnfast("font-heading text-sm font-medium", className)}
	{...rest}
>
	{@render children?.()}
</div>
