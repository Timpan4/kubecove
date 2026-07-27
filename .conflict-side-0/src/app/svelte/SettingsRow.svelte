<script lang="ts">
	import type { Snippet } from "svelte";
	import {
		Field,
		FieldDescription,
		FieldTitle,
	} from "@/components/ui/svelte";

	let {
		title,
		description,
		children,
	}: {
		title: string;
		description: string;
		children?: Snippet;
	} = $props();
</script>

<Field
	orientation="horizontal"
	class="flex-col rounded-md border bg-card p-3 sm:flex-row sm:items-center sm:justify-between sm:gap-4"
>
	<div class="min-w-0 flex-1 basis-72 space-y-1">
		<FieldTitle class="text-sm">{title}</FieldTitle>
		<FieldDescription>{description}</FieldDescription>
	</div>
	<div class="flex shrink-0 justify-start pt-1 sm:justify-end sm:pt-0">
		{@render children?.()}
	</div>
</Field>
