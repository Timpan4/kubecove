<script lang="ts">
	import { Select as SelectPrimitive } from "bits-ui";
	import type { UiProps } from "./types";

	let { children, ...rest }: UiProps & { placeholder?: string } = $props();
</script>

{#if children}
	<SelectPrimitive.Value data-slot="select-value" {...rest}>
		{@render children()}
	</SelectPrimitive.Value>
{:else}
	<SelectPrimitive.Value data-slot="select-value" {...rest} />
{/if}
