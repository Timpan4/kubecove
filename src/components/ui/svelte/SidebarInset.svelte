<script lang="ts">
	import { cnfast } from "@/lib/utils";
	import type { UiProps } from "./types";

	let { class: className = "", children, ...rest }: UiProps = $props();
</script>

<main data-slot="sidebar-inset" class={cnfast("min-w-0 flex-1", className)} {...rest}>
	{@render children?.()}
</main>
