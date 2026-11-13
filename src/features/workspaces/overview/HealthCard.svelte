<script lang="ts">
	import { Button, Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/svelte";
	import type { HealthFilter } from "@/features/resources/helpers";
	import { plural, type OverviewView } from "./view";

	let { view }: { view: OverviewView } = $props();

	const counts = $derived<Array<[label: string, value: number, filter: HealthFilter, tone: string]>>([
		["Degraded", view.health.degraded, "degraded", "text-red-300"],
		["Needs attention", view.health.attention, "attention", "text-amber-300"],
		["Healthy", view.health.healthy, "healthy", "text-emerald-300"],
		["Unknown", view.health.unknown, "unknown", ""],
		["Not evaluated", view.health.notEvaluated, "notEvaluated", ""],
		["Restart evidence", view.health.restartEvidence, "restarted", ""],
	]);
</script>

<Card size="sm" elevation="flat">
	<CardHeader class="border-b">
		<CardTitle>Resource Health</CardTitle>
		<CardDescription>
			{#if view.hasData}<span data-overview-total>{plural(view.health.total, "resource")} loaded from the saved scope.</span> Open a count to see those resources.
			{:else}Counts appear once workspace resources load.{/if}
		</CardDescription>
	</CardHeader>
	<CardContent class="grid gap-3">
		<div class="grid grid-cols-2 gap-2 @xl:grid-cols-3 @4xl:grid-cols-6">
			{#each counts as [label, value, filter, tone] (filter)}
				<Button variant="outline" class="h-auto min-h-16 flex-col items-start justify-center gap-0.5 px-3 py-2"
					disabled={!view.hasData || value === 0} onclick={() => view.onOpenResources(undefined, filter)}>
					<span class="text-[0.72rem] font-semibold uppercase text-muted-foreground">{label}</span>
					<strong class={["text-base tabular-nums", view.hasData && value > 0 && tone]}>{view.hasData ? value : "-"}</strong>
				</Button>
			{/each}
		</div>
		{@render view.loadState()}
	</CardContent>
</Card>
