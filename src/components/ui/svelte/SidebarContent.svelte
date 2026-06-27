<script lang="ts">
	import { cnfast } from "@/lib/utils";
	import type { UiProps } from "./types";

	let { class: className = "", children, ...rest }: UiProps = $props();
</script>

<div data-slot="sidebar-content" class={cnfast("flex min-h-0 flex-1 flex-col", className)} {...rest}>
	{@render children?.()}
</div>
