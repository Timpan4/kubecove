<script lang="ts">
	import { cn } from "@/lib/utils";
	import type { UiProps } from "./types";

	let { class: className = "", children, ...rest }: UiProps = $props();
</script>

<main data-slot="sidebar-inset" class={cn("min-w-0 flex-1", className)} {...rest}>
	{@render children?.()}
</main>
