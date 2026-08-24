<script lang="ts">
	// biome-ignore lint/correctness/noUnusedImports: The server-rendered Svelte markup uses this component.
	import CopyableText from "../../src/components/CopyableText.svelte";
	// biome-ignore lint/correctness/noUnusedImports: The server-rendered Svelte markup uses this provider.
	import TooltipProvider from "../../src/components/ui/svelte/TooltipProvider.svelte";

	// biome-ignore lint/correctness/noUnusedVariables: The server-rendered Svelte markup forwards these props.
	let { value, label }: { value: string; label: string } = $props();
</script>

<TooltipProvider>
	<CopyableText {value} {label} />
</TooltipProvider>
