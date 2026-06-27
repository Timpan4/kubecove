<script lang="ts">
	import { cnfast } from "@/lib/utils";
	import type { UiProps } from "./types";

	let { class: className = "", children, ...rest }: UiProps = $props();
</script>

<div data-slot="resizable-panel-group" class={cnfast("flex h-full w-full", className)} {...rest}>
	{@render children?.()}
</div>
