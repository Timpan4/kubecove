<script lang="ts">
	import Dialog from "./Dialog.svelte";
	import type { UiProps } from "./types";

	let { children, ...rest }: UiProps = $props();
</script>

<Dialog data-slot="sheet" {...rest}>
	{@render children?.()}
</Dialog>
