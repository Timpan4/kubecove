<script lang="ts">
	import { cnfast } from "@/lib/utils";
	import type { UiProps } from "./types";

	let { class: className = "", children, ...rest }: UiProps = $props();
</script>

<div data-slot="sidebar-header" data-sidebar="header" class={cnfast("flex flex-col gap-2 p-2", className)} {...rest}>
	{@render children?.()}
</div>
