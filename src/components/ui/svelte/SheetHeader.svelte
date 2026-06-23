<script lang="ts">
	import { cn } from "@/lib/utils";
	import type { UiProps } from "./types";

	let { class: className = "", children, ...rest }: UiProps = $props();
</script>

<div data-slot="sheet-header" class={cn("flex flex-col gap-1.5 p-6", className)} {...rest}>
	{@render children?.()}
</div>
