<script lang="ts">
	import { Boxes, Cable, Clock3, GitBranch, GitCompareArrows, Layers, Pin } from "lucide-svelte";
	import { Button, Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/svelte";
	import { entryLabel, type EntryRow } from "./entries";
	import { DOT, HOLLOW_DOT, plural, ROW, TONE_LABEL, type OverviewView } from "./view";

	let { view }: { view: OverviewView } = $props();
</script>

<Card size="sm" elevation="flat">
	<CardHeader class="border-b">
		<CardTitle>Return to Work</CardTitle>
		<CardDescription>Pinned and recently opened entry points, with their health now.</CardDescription>
	</CardHeader>
	<CardContent class="grid gap-3 px-1 group-data-[size=sm]/card:px-1">
		{@render Entries("Pinned", view.pinned, "Pin resources from the table or detail panel.")}
		{@render Entries("Recent", view.recent, "Recent visits will appear here.")}
		{#if view.shortcuts.length > 0}
			<section class="grid gap-1.5 px-2">
				<h2 class="font-semibold text-muted-foreground">Shortcuts</h2>
				<div class="flex flex-wrap gap-1.5">
					{#each view.shortcuts as shortcut (shortcut.id)}
						<Button variant="outline" size="sm" onclick={() => view.onOpenShortcut(shortcut)}>
							{#if shortcut.kind === "argo"}<GitBranch data-icon="inline-start" />
							{:else if shortcut.kind === "namespace"}<Layers data-icon="inline-start" />
							{:else if shortcut.kind === "compare"}<GitCompareArrows data-icon="inline-start" />
							{:else}<Boxes data-icon="inline-start" />{/if}
							{shortcut.label}
						</Button>
					{/each}
				</div>
			</section>
		{/if}
		{#if view.portForwardCount > 0}
			<div class="px-2"><Button variant="outline" size="sm" onclick={view.onOpenPortForwards}>
				<Cable data-icon="inline-start" /> {plural(view.portForwardCount, "saved port forward")}
			</Button></div>
		{/if}
	</CardContent>
</Card>

{#snippet Entries(title: "Pinned" | "Recent", rows: EntryRow[], emptyText: string)}
	<section class="grid gap-0.5">
		<h2 class="flex items-center gap-1.5 px-2 pb-1 font-semibold text-muted-foreground">
			{#if title === "Pinned"}<Pin class="size-3.5" />{:else}<Clock3 class="size-3.5" />{/if}
			{title}
		</h2>
		{#if rows.length === 0}<p class="px-2 text-muted-foreground">{emptyText}</p>{/if}
		{#each rows as row (row.key)}
			<Button variant="ghost" class={`${ROW} items-start py-1`} aria-label={entryLabel(row.entry)} onclick={() => view.onOpenEntry(row.entry)}>
				<span
					class={["mt-1.5 size-2 shrink-0 rounded-full", row.tone ? DOT[row.tone] : HOLLOW_DOT]}
					aria-hidden="true"
				></span>
				<span class="grid min-w-0">
					<span class="font-mono break-all">{row.name}</span>
					<span class="font-normal text-muted-foreground">
						{row.kind}{row.namespace ? ` in ${row.namespace}` : ""}{row.tone
							? `, ${TONE_LABEL[row.tone].toLowerCase()}`
							: ""}
					</span>
				</span>
			</Button>
		{/each}
	</section>
{/snippet}
