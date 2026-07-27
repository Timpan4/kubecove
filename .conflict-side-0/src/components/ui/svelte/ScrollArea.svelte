<script lang="ts">
	import { cnfast } from "@/lib/utils";
	import type { UiProps } from "./types";

	let { class: className = "", children, ...rest }: UiProps = $props();
</script>

<div data-slot="scroll-area" class={cnfast("overflow-auto", className)} {...rest}>
	{@render children?.()}
</div>
