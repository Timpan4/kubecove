<script lang="ts">
	import { cnfast } from "@/lib/utils";
	import type { UiProps } from "./types";

	let { class: className = "", children, ...rest }: UiProps = $props();
</script>

<div
	data-slot="card-description"
	class={cnfast("text-xs/relaxed text-muted-foreground", className)}
	{...rest}
>
	{@render children?.()}
</div>
