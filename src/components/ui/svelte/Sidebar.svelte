<script lang="ts">
	import { cn } from "@/lib/utils";
	import type { UiProps } from "./types";

	let { class: className = "", children, ...rest }: UiProps = $props();
</script>

<aside data-slot="sidebar" class={cn("bg-sidebar text-sidebar-foreground", className)} {...rest}>
	{@render children?.()}
</aside>
