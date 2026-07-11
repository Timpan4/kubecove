<script lang="ts">
	import { Card, CardDescription, CardHeader, CardTitle } from "@/components/ui/svelte";

	let { stats }: { stats: Array<[string, string | number]> } = $props();
</script>

<div class="grid gap-3 md:grid-cols-3 xl:grid-cols-5">
	{#each stats as [label, value]}
		<Card size="sm" elevation="raised">
			<CardHeader>
				<CardDescription>{label}</CardDescription>
				<CardTitle class="truncate text-xl tabular-nums">{value}</CardTitle>
			</CardHeader>
		</Card>
	{/each}
</div>
