<script lang="ts">
	import { cnfast } from "@/lib/utils";
	import type { UiProps } from "./types";

	let { class: className = "", children, ...rest }: UiProps = $props();
</script>

<aside data-slot="sidebar" class={cnfast("bg-sidebar text-sidebar-foreground", className)} {...rest}>
	{@render children?.()}
</aside>
