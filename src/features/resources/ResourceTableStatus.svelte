<script lang="ts">
	import { CircleAlert, Pause } from "lucide-svelte";
	import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/svelte";
	import HealthAssessmentBadge from "@/components/HealthAssessmentBadge.svelte";
	import { healthSourceLabel, resourceHealthAssessment } from "@/lib/resource-health";
	import type { ResourceSummary } from "@/lib/types";
	import { resourceLifecycle } from "./resource-lifecycle";
	import ResourceActivityIcon from "./ResourceActivityIcon.svelte";
	let { resource, active = true }: { resource: ResourceSummary; active?: boolean } = $props();
	const lifecycle = $derived(resourceLifecycle(resource.kind, resource.status));
	const assessment = $derived(resourceHealthAssessment(resource));
</script>

{#if lifecycle}
	<Tooltip>
		<TooltipTrigger class={`inline-flex max-w-full items-center gap-1.5 truncate rounded-sm text-xs focus-visible:outline-2 focus-visible:outline-ring ${lifecycle.tone === "info" ? "text-sky-700 dark:text-sky-300" : lifecycle.tone === "warning" ? "text-amber-700 dark:text-amber-300" : "text-destructive"}`}>
			{#if lifecycle.progress}<ResourceActivityIcon {active} />{:else if lifecycle.tone === "warning"}<Pause class="size-3 shrink-0" aria-hidden="true" />{:else}<CircleAlert class="size-3 shrink-0" aria-hidden="true" />{/if}
			<span class="truncate">{lifecycle.label}</span>
		</TooltipTrigger>
		<TooltipContent role="tooltip" class="max-w-sm flex-col items-stretch gap-2 border bg-surface-2 p-3 text-popover-foreground">
			<p class="text-xs">{resource.status}</p>
			{#if !active}<p class="text-xs text-muted-foreground">Last observed state. Activity is paused until live updates recover.</p>{/if}
			<HealthAssessmentBadge {assessment} />
			{#if assessment}
				<ul class="grid gap-1.5 text-xs text-muted-foreground">
					{#each assessment.evidence as evidence}
						<li><span class="font-medium text-foreground">{healthSourceLabel(evidence.source)}</span>: {typeof evidence.raw === "string" ? evidence.raw : JSON.stringify(evidence.raw)}. {evidence.reason}{evidence.current ? "" : " (historical)"}</li>
					{/each}
				</ul>
			{/if}
		</TooltipContent>
	</Tooltip>
{:else}
	<HealthAssessmentBadge {assessment} compact rawStatuses={resource.status ? [resource.status] : []} />
{/if}
