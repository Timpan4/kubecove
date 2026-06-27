<script lang="ts">
	import DialogTrigger from "./DialogTrigger.svelte";
	import type { UiProps } from "./types";

	let { children, ...rest }: UiProps = $props();
</script>

<DialogTrigger data-slot="sheet-trigger" {...rest}>
	{@render children?.()}
</DialogTrigger>
