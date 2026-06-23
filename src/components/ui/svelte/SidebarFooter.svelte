<script lang="ts">
	import { cn } from "@/lib/utils";
	import type { UiProps } from "./types";

	let { class: className = "", children, ...rest }: UiProps = $props();
</script>

<div data-slot="sidebar-footer" data-sidebar="footer" class={cn("flex flex-col gap-2 p-2", className)} {...rest}>
	{@render children?.()}
</div>
