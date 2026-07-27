<script lang="ts">
	import { Badge } from "@/components/ui/svelte";
	import type { IncidentCockpitItem } from "@/lib/types";
	import { cnfast } from "@/lib/utils";
	import {
		incidentItemKey,
		incidentScopeLabel,
		incidentSeverityLabel,
		incidentSignalSummary,
	} from "./model";

	let {
		groups,
		selectedKey,
		visibleCount,
		onSelect,
	}: {
		groups: Array<{ label: string; items: IncidentCockpitItem[] }>;
		selectedKey: string | null;
		visibleCount: number;
		onSelect: (key: string) => void;
	} = $props();

	function resourceLabel(item: IncidentCockpitItem): string {
		return `${item.resource.kind}/${item.resource.name}`;
	}

	function severityClass(item: IncidentCockpitItem): string {
		if (item.severity === "degraded") return "border-l-destructive";
		if (item.severity === "attention") return "border-l-amber-500";
		if (item.severity === "restarted") return "border-l-sky-500";
		return "border-l-muted-foreground";
	}
</script>

<section class="flex min-w-0 flex-col rounded-md border bg-surface-1 @4xl:h-full @4xl:min-h-0">
	<header class="flex items-start justify-between gap-3 border-b px-3 py-3">
		<div class="min-w-0">
			<h3 class="text-sm font-semibold">Signal queue</h3>
			<p class="mt-1 text-xs text-muted-foreground">
				Grouped by ownership, sorted by severity and recency.
			</p>
		</div>
		<Badge variant="secondary" class="tabular-nums">{visibleCount}</Badge>
	</header>

	<div
		class="max-h-[38vh] min-h-0 overflow-y-auto p-3 @4xl:max-h-none @4xl:flex-1"
		aria-label="Incident signals"
	>
		<div class="flex flex-col gap-3">
			{#each groups as group}
				<section class="flex flex-col gap-2">
					<div class="truncate text-xs font-semibold uppercase text-muted-foreground">
						{group.label}
					</div>
					{#each group.items as item (incidentItemKey(item))}
						{@const key = incidentItemKey(item)}
						{@const selected = key === selectedKey}
						<button
							type="button"
							aria-pressed={selected}
							aria-current={selected ? "true" : undefined}
							class={cnfast(
								"w-full rounded-md border border-l-4 bg-background/35 p-3 text-left transition-[background-color,border-color,transform] duration-150 hover:border-primary/50 hover:bg-primary/5 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring active:scale-[0.99] motion-reduce:transition-none motion-reduce:active:scale-100",
								severityClass(item),
								selected && "border-primary bg-primary/10",
							)}
							onclick={() => onSelect(key)}
						>
							<div class="flex min-w-0 items-start justify-between gap-3">
								<div class="min-w-0">
									<div class="truncate text-sm font-semibold">{resourceLabel(item)}</div>
									<div class="mt-1 truncate text-xs text-muted-foreground">
										{incidentScopeLabel(item)}
									</div>
								</div>
								<Badge variant={item.severity === "degraded" ? "destructive" : "outline"}>
									{incidentSeverityLabel(item)}
								</Badge>
							</div>
							<div class="mt-2 flex flex-wrap gap-1.5">
								{#if item.resource.status}<Badge variant="outline">{item.resource.status}</Badge>{/if}
								{#if item.resource.ready}<Badge variant="outline">Ready {item.resource.ready}</Badge>{/if}
								{#if item.resource.restarts && item.resource.restarts > 0}
									<Badge variant="outline">{item.resource.restarts} restarts</Badge>
								{/if}
								{#if item.warningEventCount > 0}
									<Badge variant="outline">{item.warningEventCount} warnings</Badge>
								{/if}
							</div>
							<p class="mt-2 line-clamp-2 text-xs leading-relaxed text-muted-foreground">
								{incidentSignalSummary(item)}
							</p>
						</button>
					{/each}
				</section>
			{/each}
		</div>
	</div>
</section>
