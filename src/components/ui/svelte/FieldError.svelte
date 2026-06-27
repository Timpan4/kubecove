<script lang="ts">
	import { cnfast } from "@/lib/utils";
	import type { UiFieldError, UiProps } from "./types";

	let {
		class: className = "",
		children,
		errors,
		...rest
	}: UiProps & { errors?: Array<UiFieldError | undefined> } = $props();

	const messages = $derived(
		errors
			? Array.from(
					new Set(
						errors.flatMap((error) => (error?.message ? [error.message] : [])),
					),
				)
			: [],
	);
</script>

{#if children || messages.length > 0}
	<div
		role="alert"
		data-slot="field-error"
		class={cnfast("text-xs/relaxed font-normal text-destructive", className)}
		{...rest}
	>
		{#if children}
			{@render children()}
		{:else if messages.length === 1}
			{messages[0]}
		{:else}
			<ul class="ml-4 flex list-disc flex-col gap-1">
				{#each messages as message}
					<li>{message}</li>
				{/each}
			</ul>
		{/if}
	</div>
{/if}
