<script lang="ts">
	import { Button } from "@/components/ui/svelte";
	import { buildArgoResourceDiff, diffLineClassName } from "@/features/resource-detail/yamlTabDiff";
	import { COMPACT_ARGO_DIFF_LINES, highlightedArgoDiffSegments } from "./argo-resource-display";

	let { desired, live }: { desired: string; live: string } = $props();
	let expanded = $state(false);
	const lines = $derived(buildArgoResourceDiff(desired, live));
	const visibleLines = $derived(expanded ? lines : lines.slice(0, COMPACT_ARGO_DIFF_LINES));
	const hiddenChangedCount = $derived(lines.slice(COMPACT_ARGO_DIFF_LINES).filter((line) => line.type === "add" || line.type === "remove").length);
</script>

{#each visibleLines as line}
	<div class={`whitespace-pre px-3 ${diffLineClassName(line.type)}`}>{#each highlightedArgoDiffSegments(line) as segment}<span class={segment.className}>{segment.text}</span>{/each}</div>
{/each}
{#if lines.length > COMPACT_ARGO_DIFF_LINES}
	{#if !expanded}<span class="px-3 font-sans text-xs text-muted-foreground">{lines.length - COMPACT_ARGO_DIFF_LINES} more lines ({hiddenChangedCount} changed) hidden.</span>{/if}
	<Button type="button" variant="ghost" size="sm" aria-expanded={expanded} onclick={() => (expanded = !expanded)}>
		{expanded ? "Show fewer lines" : `Show all ${lines.length} lines`}
	</Button>
{/if}
