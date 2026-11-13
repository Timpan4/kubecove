<script lang="ts">
	import { Button, Card, CardAction, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/svelte";
	import type { NamespaceRow } from "./model";
	import { DOT, HOLLOW_DOT, plural, ROW, type OverviewView } from "./view";
	let { view }: { view: OverviewView } = $props();
	let showQuiet = $state(false);
	const findings = (row: NamespaceRow) => row.degraded > 0 || row.attention > 0;
	const flagged = $derived(view.model.namespaces.filter(findings));
	const quiet = $derived(view.model.namespaces.filter(row => !findings(row)));
	const quietOpen = $derived(showQuiet || flagged.length === 0);
</script>
{#if view.model.namespaces.length > 0}
	<Card size="sm" elevation="flat">
		<CardHeader class="border-b">
			<CardTitle>Namespaces</CardTitle>
			<CardDescription>{flagged.length} namespaces with findings, of {view.model.namespaces.length} loaded.</CardDescription>
			{#if quiet.length > 0 && flagged.length > 0}
				<CardAction><Button variant="outline" size="sm" aria-expanded={quietOpen} onclick={() => showQuiet = !showQuiet}>
					{quietOpen ? "Hide" : "Show"} {quiet.length} without findings
				</Button></CardAction>
			{/if}
		</CardHeader>
		<CardContent class="grid grid-cols-[repeat(auto-fill,minmax(min(18rem,100%),1fr))] gap-x-3 gap-y-0.5 px-1 group-data-[size=sm]/card:px-1">
			{#each flagged as row (row.key)}{@render Namespace(row)}{/each}
			{#if quietOpen}{#each quiet as row (row.key)}{@render Namespace(row)}{/each}{/if}
		</CardContent>
	</Card>
{/if}
{#snippet Namespace(row: NamespaceRow)}
	<Button variant="ghost" class={ROW} aria-label={`Open namespace ${row.namespace}`}
		onclick={() => view.onOpenResources(row.namespace, row.degraded > 0 ? "degraded" : row.attention > 0 ? "attention" : undefined)}>
		<span class={["size-2 shrink-0 rounded-full", row.degraded > 0 ? DOT.degraded : row.attention > 0 ? DOT.needsAttention : HOLLOW_DOT]} aria-hidden="true"></span>
		<span class="min-w-0 flex-1 font-mono break-all">{row.namespace}</span>
		{#if row.degraded > 0}<span class="text-red-300 tabular-nums">{row.degraded} degraded</span>{/if}
		{#if row.attention > 0}<span class="text-amber-300 tabular-nums">{row.attention} attention</span>{/if}
		<span class="font-normal text-muted-foreground tabular-nums">{findings(row) ? `of ${row.total}` : plural(row.total, "resource")}</span>
	</Button>
{/snippet}
