<script lang="ts">
	import { Check, Copy } from "lucide-svelte";
	import HealthAssessmentBadge from "@/components/HealthAssessmentBadge.svelte";
	import { Badge, Button } from "@/components/ui/svelte";
	import type { IncidentCockpitItem } from "@/lib/types";
	import { cnfast } from "@/lib/utils";
	import {
		incidentItemKey,
		incidentScopeLabel,
		incidentSeverityLabel,
		incidentSignalSummary,
		incidentState,
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
	let copyStatus = $state<{ key: string; state: "copied" | "failed" } | null>(null);

	function resourceLabel(item: IncidentCockpitItem): string {
		return `${item.resource.kind}/${item.resource.name}`;
	}

	function severityClass(item: IncidentCockpitItem): string {
		if (incidentState(item) === "historical") return "border-l-muted-foreground";
		if (incidentState(item) === "resolved") return "border-l-sky-500";
		if (item.severity === "degraded") return "border-l-destructive";
		if (item.severity === "attention") return "border-l-amber-500";
		if (item.severity === "restarted") return "border-l-amber-500";
		return "border-l-muted-foreground";
	}

	async function copyResourceName(item: IncidentCockpitItem): Promise<void> {
		const key = incidentItemKey(item);
		try {
			await navigator.clipboard.writeText(item.resource.name);
			copyStatus = { key, state: "copied" };
		} catch {
			copyStatus = { key, state: "failed" };
		}
	}
</script>

<section class="flex min-w-0 flex-col rounded-md border bg-surface-1 @5xl:h-full @5xl:min-h-0">
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
		data-incident-queue-scroll
		class="min-h-0 p-3 @5xl:flex-1 @5xl:overflow-y-auto"
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
						<article
							class={cnfast(
								"overflow-hidden rounded-md border border-l-4 bg-background/35 transition-[background-color,border-color] duration-150",
								severityClass(item),
								selected && "border-primary bg-primary/10",
							)}
						>
							<button
								type="button"
								aria-pressed={selected}
								aria-current={selected ? "true" : undefined}
								class="w-full p-3 text-left transition-colors duration-150 hover:bg-primary/5 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring motion-reduce:transition-none"
								onclick={() => onSelect(key)}
							>
								<div class="flex min-w-0 items-start justify-between gap-3">
									<div class="min-w-0">
										<div class="break-words text-sm font-semibold">{resourceLabel(item)}</div>
										<div class="mt-1 break-words text-xs text-muted-foreground">
											{incidentScopeLabel(item)}
										</div>
									</div>
									<Badge class="shrink-0" variant={item.severity === "degraded" ? "destructive" : "outline"}>
										{incidentSeverityLabel(item)}
									</Badge>
								</div>
								<div class="mt-2">
									<HealthAssessmentBadge assessment={item.resource.healthAssessment} />
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
							<div class="flex justify-end border-t px-2 py-1.5">
								<Button
									type="button"
									variant="ghost"
									size="sm"
									aria-label={`Copy resource name ${item.resource.name}`}
									onclick={() => void copyResourceName(item)}
								>
									{#if copyStatus?.key === key && copyStatus.state === "copied"}
										<Check data-icon="inline-start" /> Copied
									{:else}
										<Copy data-icon="inline-start" />
										{copyStatus?.key === key && copyStatus.state === "failed" ? "Copy failed" : "Copy name"}
									{/if}
								</Button>
							</div>
						</article>
					{/each}
				</section>
			{/each}
		</div>
	</div>
</section>
