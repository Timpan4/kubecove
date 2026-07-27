<script lang="ts">
	import { cnfast } from "@/lib/utils";
	import type { UiProps } from "./types";

	let { class: className = "", children, ...rest }: UiProps = $props();
</script>

<ul data-slot="sidebar-menu" data-sidebar="menu" class={cnfast("flex w-full min-w-0 flex-col gap-px", className)} {...rest}>
	{@render children?.()}
</ul>
