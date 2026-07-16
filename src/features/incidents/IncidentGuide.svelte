<script lang="ts">
	import { ArrowRight, ExternalLink, ShieldAlert } from "lucide-svelte";
	import {
		Badge,
		Button,
		Empty,
		EmptyDescription,
		EmptyHeader,
		EmptyTitle,
		Spinner,
	} from "@/components/ui/svelte";
	import type { PathStateDetailTab } from "@/lib/path-state";
	import type { IncidentCockpitItem, ResourceSummary } from "@/lib/types";
	import { cnfast } from "@/lib/utils";
	import type {
		IncidentEnrichmentState,
		IncidentEvidenceTone,
		IncidentGuidance,
	} from "./guidance";
	import { incidentScopeLabel, incidentSeverityLabel } from "./model";

	let {
		selectedIncident,
		guidance,
		detailsState,
		topologyState,
		onResourceInspect,
		onResourceSelect,
	}: {
		selectedIncident: IncidentCockpitItem | null;
		guidance: IncidentGuidance | null;
		detailsState: IncidentEnrichmentState;
		topologyState: IncidentEnrichmentState;
		onResourceInspect: (resource: ResourceSummary, detailTab?: PathStateDetailTab) => void;
		onResourceSelect: (resource: ResourceSummary) => void;
	} = $props();

	const enriching = $derived(detailsState === "loading" || topologyState === "loading");
	const enrichmentFailed = $derived(detailsState === "error" || topologyState === "error");
	let showStepContext = $state(false);

	function resourceLabel(resource: ResourceSummary): string {
		return `${resource.kind}/${resource.name}`;
	}

	function evidenceClass(tone: IncidentEvidenceTone): string {
		if (tone === "error") return "border-l-destructive";
		if (tone === "warning") return "border-l-amber-500";
		if (tone === "info") return "border-l-sky-500";
		return "border-l-muted-foreground";
	}
</script>

<section class="min-w-0 rounded-md border bg-surface-1 @4xl:h-full @4xl:min-h-0 @4xl:overflow-y-auto">
	{#if selectedIncident && guidance}
		<header class="border-b px-4 py-3">
			<div class="flex flex-wrap items-start justify-between gap-3">
				<div class="min-w-0">
					<div class="mb-1 text-xs font-semibold uppercase text-muted-foreground">Guided investigation</div>
					<h3 class="break-words text-base font-semibold leading-snug">{guidance.title}</h3>
					<p class="mt-1 text-xs text-muted-foreground">{incidentScopeLabel(selectedIncident)}</p>
				</div>
				<div class="flex flex-wrap items-center gap-2">
					<Badge variant={selectedIncident.severity === "degraded" ? "destructive" : "outline"}>
						{incidentSeverityLabel(selectedIncident)}
					</Badge>
					<Button
						type="button"
						variant="outline"
						size="sm"
						onclick={() => onResourceSelect(selectedIncident.resource)}
					>
						<ExternalLink data-icon="inline-start" />
						Open in Resources
					</Button>
				</div>
			</div>
			<p class="mt-3 max-w-4xl text-sm leading-relaxed">{guidance.summary}</p>
		</header>

		<div class="grid gap-3 p-4">
			{#if enriching || enrichmentFailed}
				<div
					class={cnfast(
						"flex items-center gap-2 rounded-md border px-3 py-2 text-xs",
						enrichmentFailed
							? "border-amber-500/40 bg-amber-500/5 text-amber-700 dark:text-amber-300"
							: "bg-background/35 text-muted-foreground",
					)}
				>
					{#if enriching}<Spinner />{/if}
					{enrichmentFailed
						? "Some live evidence is unavailable. Snapshot guidance remains usable."
						: "Loading live conditions and controller ownership…"}
				</div>
			{/if}

			<div class="grid gap-2 @2xl:grid-cols-3">
				{#each guidance.facts as fact}
					<section class="rounded-md border bg-background/35 p-3">
						<div class="text-xs font-semibold uppercase text-muted-foreground">{fact.label}</div>
						<div class="mt-1 break-words text-sm font-semibold">{fact.value}</div>
						<p class="mt-1 text-xs leading-relaxed text-muted-foreground">{fact.detail}</p>
					</section>
				{/each}
			</div>

			<section class="rounded-md border bg-background/35">
				<header class="border-b px-3 py-2.5">
					<h4 class="text-xs font-semibold uppercase text-muted-foreground">Confirmed evidence</h4>
				</header>
				{#if guidance.evidence.length > 0}
					<div class="grid gap-2 p-3">
						{#each guidance.evidence as evidence (evidence.id)}
							<div class={cnfast("rounded-md border border-l-4 bg-surface-1 px-3 py-2", evidenceClass(evidence.tone))}>
								<div class="flex flex-wrap items-start justify-between gap-2">
									<div class="text-xs font-semibold">{evidence.label}</div>
									{#if evidence.timestamp}
										<span class="text-[0.625rem] tabular-nums text-muted-foreground">{evidence.timestamp}</span>
									{/if}
								</div>
								<p class="mt-1 break-words text-xs leading-relaxed text-muted-foreground">{evidence.detail}</p>
							</div>
						{/each}
					</div>
				{:else}
					<p class="p-3 text-xs text-muted-foreground">No detailed evidence is available in this snapshot.</p>
				{/if}
			</section>

			<section class="rounded-md border border-amber-500/30 bg-amber-500/5 p-3">
				<h4 class="text-xs font-semibold uppercase text-amber-700 dark:text-amber-300">Still unknown</h4>
				<ul class="mt-2 grid gap-1.5 text-xs leading-relaxed text-muted-foreground">
					{#each guidance.missing as missing}
						<li class="flex gap-2"><span aria-hidden="true">•</span><span>{missing}</span></li>
					{/each}
				</ul>
			</section>

			<section class="rounded-md border bg-background/35 p-3">
				<div class="mb-3 flex flex-wrap items-start justify-between gap-2">
					<div>
						<h4 class="text-xs font-semibold uppercase text-muted-foreground">Next investigation steps</h4>
						{#if showStepContext}
							<p class="mt-1 text-xs text-muted-foreground">Open evidence in the detail pane without losing queue selection.</p>
						{/if}
					</div>
					<Button
						type="button"
						variant="ghost"
						size="sm"
						aria-pressed={showStepContext}
						onclick={() => (showStepContext = !showStepContext)}
					>
						{showStepContext ? "Hide context" : "Show context"}
					</Button>
				</div>
				<div class={showStepContext ? "grid gap-2 @2xl:grid-cols-2" : "flex flex-wrap gap-2"}>
					{#each guidance.steps as step (step.id)}
						{#if showStepContext}
							<div class="flex min-w-0 flex-col justify-between gap-3 rounded-md border bg-surface-1 p-3">
								<div class="min-w-0">
									<div class="flex flex-wrap items-center gap-2">
										<div class="text-xs font-semibold">{step.title}</div>
										<Badge variant="outline" class="max-w-full truncate">{resourceLabel(step.target)}</Badge>
									</div>
									<p class="mt-1 text-xs leading-relaxed text-muted-foreground">{step.description}</p>
								</div>
								<Button
									type="button"
									variant="outline"
									size="sm"
									class="self-start"
									onclick={() => onResourceInspect(step.target, step.tab)}
								>
									Open {step.tab === "details" ? "details" : step.tab}
									<ArrowRight data-icon="inline-end" />
								</Button>
							</div>
						{:else}
							<Button
								type="button"
								variant="outline"
								size="sm"
								onclick={() => onResourceInspect(step.target, step.tab)}
							>
								{step.title}
								<ArrowRight data-icon="inline-end" />
							</Button>
						{/if}
					{/each}
				</div>
			</section>

			{#if guidance.actions.length > 0}
				<section class="rounded-md border bg-background/35 p-3">
					<div class="mb-3 flex items-start gap-2">
						<ShieldAlert class="mt-0.5 size-4 shrink-0 text-muted-foreground" />
						<div>
							<h4 class="text-xs font-semibold uppercase text-muted-foreground">Available guarded actions</h4>
							<p class="mt-1 text-xs text-muted-foreground">
								Choices, not recommendations. Each opens a preview-and-confirm flow; nothing runs here.
							</p>
						</div>
					</div>
					<div class="grid gap-2 @2xl:grid-cols-2">
						{#each guidance.actions as action (action.id)}
							<div
								class={cnfast(
									"flex min-w-0 flex-col justify-between gap-3 rounded-md border bg-surface-1 p-3",
									action.tone === "destructive" && "border-destructive/35 bg-destructive/5",
								)}
							>
								<div>
									<div class="text-xs font-semibold">{action.label}</div>
									<p class="mt-1 text-xs leading-relaxed text-muted-foreground">{action.description}</p>
								</div>
								<Button
									type="button"
									variant={action.tone === "destructive" ? "destructive" : "outline"}
									size="sm"
									class="self-start"
									onclick={() => onResourceInspect(action.target, "operations")}
								>
									Review in Actions
									<ArrowRight data-icon="inline-end" />
								</Button>
							</div>
						{/each}
					</div>
				</section>
			{/if}
		</div>
	{:else if selectedIncident}
		<Empty class="min-h-80 border-0 bg-transparent">
			<EmptyHeader>
				<Spinner class="mx-auto" />
				<EmptyTitle>Preparing guided investigation</EmptyTitle>
				<EmptyDescription>Incident snapshot remains selected while supporting evidence loads.</EmptyDescription>
			</EmptyHeader>
		</Empty>
	{:else}
		<Empty class="min-h-80 border-0 bg-transparent">
			<EmptyHeader>
				<EmptyTitle>Choose an incident signal</EmptyTitle>
				<EmptyDescription>Select a queue item to build a guided investigation.</EmptyDescription>
			</EmptyHeader>
		</Empty>
	{/if}
</section>
