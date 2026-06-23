<script lang="ts">
	import { cn } from "@/lib/utils";
	import type { UiProps } from "./types";

	let { class: className = "", ...rest }: UiProps = $props();
</script>

<div
	data-slot="resizable-handle"
	class={cn("w-px shrink-0 bg-border", className)}
	{...rest}
></div>
