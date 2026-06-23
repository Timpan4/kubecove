<script lang="ts">
	import { cn } from "@/lib/utils";
	import type { UiProps } from "./types";

	let { class: className = "", children, ...rest }: UiProps = $props();
</script>

<div
	data-slot="sidebar-group"
	data-sidebar="group"
	class={cn("relative flex w-full min-w-0 flex-col px-2 py-1", className)}
	{...rest}
>
	{@render children?.()}
</div>
