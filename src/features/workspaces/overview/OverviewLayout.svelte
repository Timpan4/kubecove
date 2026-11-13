<script lang="ts">
	import { Button, Card, CardAction, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/svelte";

	import HealthCard from "./HealthCard.svelte";
	import NamespaceCard from "./NamespaceCard.svelte";
	import ProblemEmpty from "./ProblemEmpty.svelte";
	import ProblemTable from "./ProblemTable.svelte";
	import { plural, type OverviewView } from "./view";
	import ReturnToWork from "./ReturnToWork.svelte";

	let { view }: { view: OverviewView } = $props();
</script>

<HealthCard {view} />

<div class="grid gap-4 @6xl:grid-cols-[minmax(0,1fr)_20rem] @6xl:items-start">
	<div class="grid min-w-0 gap-4">
		<Card size="sm" elevation="flat">
			<CardHeader class="border-b">
				<CardTitle>Needs attention</CardTitle>
				<CardDescription>
					{#if !view.hasData}
						Listed once workspace resources load.
					{:else if view.model.problems.length > 0}
						{plural(view.model.problems.length, "finding")} behind the
						{plural(view.model.problemResources, "resource")} counted as degraded or needing attention.
					{:else}
						Degraded and needs-attention resources, folded into the workload that owns them.
					{/if}
				</CardDescription>
				<CardAction>
					<Button variant="outline" size="sm" onclick={() => view.onOpenIncidents()}>Open incident queue</Button>
				</CardAction>
			</CardHeader>
			{#if view.model.problems.length > 0}
				<CardContent class="px-0 group-data-[size=sm]/card:px-0">
					<ProblemTable {view} problems={view.model.problems} />
				</CardContent>
			{:else}
				<ProblemEmpty {view} />
			{/if}
		</Card>
		<NamespaceCard {view} />
	</div>

	<div class="grid min-w-0 gap-4 @2xl:grid-cols-2 @2xl:items-start @6xl:grid-cols-1">
		<ReturnToWork {view} />
		{@render view.gitOps()}
	</div>
</div>
