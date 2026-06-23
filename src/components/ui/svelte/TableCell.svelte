<script lang="ts">
	import { cn } from "@/lib/utils";
	import type { UiProps } from "./types";

	let { class: className = "", children, ...rest }: UiProps = $props();
</script>

<td
	data-slot="table-cell"
	class={cn(
		"p-2 align-middle whitespace-nowrap [&:has([role=checkbox])]:pr-0",
		className,
	)}
	{...rest}
>
	{@render children?.()}
</td>
