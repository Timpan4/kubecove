<script lang="ts">
	import { cnfast } from "@/lib/utils";
	import type { UiProps } from "./types";

	let {
		class: className = "",
		children,
		scrollTop = $bindable(0),
		...rest
	}: UiProps & { scrollTop?: number } = $props();
	let scrollElement = $state<HTMLDivElement>();

	$effect(() => {
		if (scrollElement && scrollElement.scrollTop !== scrollTop) {
			scrollElement.scrollTop = scrollTop;
		}
	});
</script>

<div
	data-slot="scroll-area"
	class={cnfast("overflow-auto", className)}
	bind:this={scrollElement}
	onscroll={(event) => (scrollTop = event.currentTarget.scrollTop)}
	{...rest}
>
	{@render children?.()}
</div>
