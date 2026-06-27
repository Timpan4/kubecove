<script lang="ts">
	import { cnfast } from "@/lib/utils";
	import type { UiProps } from "./types";

	let { class: className = "", children, ...rest }: UiProps = $props();
</script>

<div data-slot="sidebar-group-content" data-sidebar="group-content" class={cnfast("w-full text-xs", className)} {...rest}>
	{@render children?.()}
</div>
