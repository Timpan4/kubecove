<script lang="ts">
	import { cn } from "@/lib/utils";
	import type { UiProps } from "./types";

	let { class: className = "", children, ...rest }: UiProps = $props();
</script>

<div data-slot="sidebar-content" class={cn("flex min-h-0 flex-1 flex-col", className)} {...rest}>
	{@render children?.()}
</div>
