<script lang="ts">
	import { cnfast } from "@/lib/utils";
	import type { UiProps } from "./types";

	let { class: className = "", children, ...rest }: UiProps = $props();
</script>

<div
	data-slot="card-content"
	class={cnfast("px-4 group-data-[size=sm]/card:px-3", className)}
	{...rest}
>
	{@render children?.()}
</div>
