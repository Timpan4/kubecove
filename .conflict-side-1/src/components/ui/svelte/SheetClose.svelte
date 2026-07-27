<script lang="ts">
	import DialogClose from "./DialogClose.svelte";
	import type { UiProps } from "./types";

	let { children, ...rest }: UiProps = $props();
</script>

<DialogClose data-slot="sheet-close" {...rest}>
	{@render children?.()}
</DialogClose>
