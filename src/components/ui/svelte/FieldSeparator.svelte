<script lang="ts">
	import { cnfast } from "@/lib/utils";
	import Separator from "./Separator.svelte";
	import type { UiProps } from "./types";

	let { class: className = "", children, ...rest }: UiProps = $props();
</script>

<div
	data-slot="field-separator"
	data-content={Boolean(children)}
	class={cnfast(
		"relative -my-2 h-5 text-xs/relaxed group-data-[variant=outline]/field-group:-mb-2",
		className,
	)}
	{...rest}
>
	<Separator class="absolute inset-0 top-1/2" />
	{#if children}
		<span
			class="relative mx-auto block w-fit bg-background px-2 text-muted-foreground"
			data-slot="field-separator-content"
		>
			{@render children()}
		</span>
	{/if}
</div>
