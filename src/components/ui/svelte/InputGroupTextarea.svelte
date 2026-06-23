<script lang="ts">
	import Textarea from "./Textarea.svelte";
	import { cn } from "@/lib/utils";
	import type { UiProps } from "./types";

	let {
		class: className = "",
		value = $bindable(""),
		...rest
	}: UiProps & { value?: string } = $props();
</script>

<Textarea
	data-slot="input-group-control"
	class={cn(
		"flex-1 resize-none rounded-none border-0 bg-transparent py-2 shadow-none ring-0 focus-visible:ring-0 aria-invalid:ring-0 dark:bg-transparent",
		className,
	)}
	bind:value
	{...rest}
/>
