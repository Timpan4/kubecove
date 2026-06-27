<script lang="ts">
	import { cnfast } from "@/lib/utils";
	import type { UiProps } from "./types";

	let { class: className = "", children, ...rest }: UiProps = $props();
</script>

<div
	data-slot="alert-action"
	class={cnfast("absolute top-1.5 right-2", className)}
	{...rest}
>
	{@render children?.()}
</div>
