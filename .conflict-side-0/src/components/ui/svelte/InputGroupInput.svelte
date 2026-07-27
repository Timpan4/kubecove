<script lang="ts">
	import Input from "./Input.svelte";
	import { cnfast } from "@/lib/utils";
	import type { UiProps } from "./types";

	let {
		class: className = "",
		value = $bindable(""),
		...rest
	}: UiProps & { value?: string } = $props();
</script>

<Input
	data-slot="input-group-control"
	class={cnfast(
		"flex-1 rounded-none border-0 bg-transparent shadow-none ring-0 focus-visible:ring-0 aria-invalid:ring-0 dark:bg-transparent",
		className,
	)}
	bind:value
	{...rest}
/>
