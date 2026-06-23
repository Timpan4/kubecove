<script lang="ts">
	import { cn } from "@/lib/utils";
	import type { UiProps } from "./types";

	let { class: className = "", children, ...rest }: UiProps = $props();
</script>

<div data-slot="table-container" class="relative w-full overflow-x-auto">
	<table data-slot="table" class={cn("w-full caption-bottom text-xs", className)} {...rest}>
		{@render children?.()}
	</table>
</div>
