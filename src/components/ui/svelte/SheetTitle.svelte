<script lang="ts">
	import DialogTitle from "./DialogTitle.svelte";
	import type { UiProps } from "./types";

	let { children, ...rest }: UiProps = $props();
</script>

<DialogTitle data-slot="sheet-title" {...rest}>
	{@render children?.()}
</DialogTitle>
