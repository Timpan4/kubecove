<script lang="ts">
	import CopyableText from "../../src/components/CopyableText.svelte";
	import TooltipProvider from "../../src/components/ui/svelte/TooltipProvider.svelte";

	let { value, label }: { value: string; label: string } = $props();
</script>

<TooltipProvider>
	<CopyableText {value} {label} />
</TooltipProvider>
