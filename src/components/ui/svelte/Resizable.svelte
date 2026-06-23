<script lang="ts">
	import { cn } from "@/lib/utils";
	import type { UiProps } from "./types";

	let { class: className = "", children, ...rest }: UiProps = $props();
</script>

<div data-slot="resizable-panel-group" class={cn("flex h-full w-full", className)} {...rest}>
	{@render children?.()}
</div>
