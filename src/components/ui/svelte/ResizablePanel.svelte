<script lang="ts">
	import { cnfast } from "@/lib/utils";
	import type { UiProps } from "./types";

	let { class: className = "", children, ...rest }: UiProps = $props();
</script>

<div data-slot="resizable-panel" class={cnfast("min-h-0 min-w-0", className)} {...rest}>
	{@render children?.()}
</div>
