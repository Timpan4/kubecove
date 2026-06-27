<script lang="ts">
	import { cnfast } from "@/lib/utils";
	import type { UiProps } from "./types";

	let { class: className = "", children, ...rest }: UiProps = $props();
</script>

<caption
	data-slot="table-caption"
	class={cnfast("mt-4 text-xs text-muted-foreground", className)}
	{...rest}
>
	{@render children?.()}
</caption>
