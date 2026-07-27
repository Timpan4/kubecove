<script lang="ts">
	import DialogDescription from "./DialogDescription.svelte";
	import type { UiProps } from "./types";

	let { children, ...rest }: UiProps = $props();
</script>

<DialogDescription data-slot="sheet-description" {...rest}>
	{@render children?.()}
</DialogDescription>
