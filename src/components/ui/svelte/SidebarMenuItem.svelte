<script lang="ts">
	import { cn } from "@/lib/utils";
	import type { UiProps } from "./types";

	let { class: className = "", children, ...rest }: UiProps = $props();
</script>

<li data-slot="sidebar-menu-item" data-sidebar="menu-item" class={cn("group/menu-item relative", className)} {...rest}>
	{@render children?.()}
</li>
