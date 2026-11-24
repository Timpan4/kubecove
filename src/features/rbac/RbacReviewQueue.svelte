<script lang="ts">
	import { Badge, Button, Card } from "@/components/ui/svelte";
	import type { RbacInspectionSummary } from "@/lib/types";
	import { cnfast } from "@/lib/utils";
	import type { RbacBindingIndex, RbacCockpitItem } from "./cockpitModel";
	import { queueSummary, queueTags } from "./queueModel";
	import RbacReviewBadge from "./RbacReviewBadge.svelte";
	import type { RbacReviewRecord } from "./reviewModel";
	import { riskBadgeClass, riskRailClass, riskSummaryLabel } from "./risk";
	import type { RbacView } from "./surfaceModel";
	let { inspection, bindingIndex, view, items, selected, visible = $bindable(), select, reviewRecords }: {
		inspection: RbacInspectionSummary;
		bindingIndex: RbacBindingIndex;
		view: RbacView;
		items: RbacCockpitItem[];
		selected: RbacCockpitItem | undefined;
		visible: number;
		select: (key: string) => void;
		reviewRecords: RbacReviewRecord[];
	} = $props();

</script>

<Card size="sm" elevation="flat" class="min-h-0 gap-0 py-0">
	<div class="flex items-start gap-3 border-b border-border px-3 py-3">
		<div class="min-w-0 flex-1">
			<p class="text-sm font-semibold">{view === "Service Accounts" ? "Service account review queue" : `${view} review queue`}</p>
			<p class="mt-0.5 text-xs text-muted-foreground">Highest risk · most findings · namespace and name</p>
		</div>
		<Badge variant="secondary" class="tabular-nums">{items.length}</Badge>
	</div>
	<div class="min-h-0 flex-1 space-y-2 overflow-y-auto p-2">
		{#each items.slice(0, visible) as item (item.key)}
			<button
				class={cnfast(
					"w-full rounded-md border border-l-[3px] bg-card p-3 text-left transition-colors hover:bg-muted/60 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring active:scale-[.995] motion-reduce:transition-none",
					riskRailClass(item.risks),
					selected?.key === item.key && "border-primary/70 bg-primary/5 ring-1 ring-primary/40",
				)}
				aria-pressed={selected?.key === item.key}
				onclick={() => select(item.key)}
			>
				<div class="flex items-start gap-2">
					<div class="min-w-0 flex-1">
						<p class="truncate text-sm font-semibold">{item.kind}/{item.name}</p>
						<p class="truncate text-xs text-muted-foreground">{item.namespace ?? "cluster scope"}</p>
					</div>
					<div class="flex shrink-0 flex-col items-end gap-1">
						<Badge variant="outline" class={riskBadgeClass(item.risks)}>{riskSummaryLabel(item.risks)}</Badge>
						<RbacReviewBadge {inspection} {bindingIndex} entry={item} records={reviewRecords} />
					</div>
				</div>
				<div class="mt-2 flex flex-wrap gap-1">
					{#each queueTags(bindingIndex, item) as tag}
						<Badge variant="outline" class="h-4 rounded-sm px-1.5 font-normal">{tag}</Badge>
					{/each}
				</div>
				<p class="mt-2 truncate text-xs text-muted-foreground">{queueSummary(item)}</p>
			</button>
		{:else}
			<div class="p-6 text-center text-sm text-muted-foreground">No matching RBAC objects.</div>
		{/each}
		{#if visible < items.length}
			<Button class="w-full" variant="outline" size="sm" onclick={() => (visible += 50)}>Load 50 more</Button>
		{/if}
	</div>
</Card>
