<script lang="ts">
	import type { HealthAssessment } from "@/lib/types";
	import {
		healthSourceLabel,
		healthSourceSummary,
		healthStateLabel,
	} from "@/lib/resource-health";
	import { Badge } from "@/components/ui/svelte";

	let {
		assessment,
		loading = false,
		details = false,
	}: { assessment?: HealthAssessment | null; loading?: boolean; details?: boolean } = $props();

	const state = $derived(assessment?.state);
	const label = $derived(
		loading ? "Loading" : assessment ? healthStateLabel(assessment.state) : "Assessment unavailable",
	);
	const source = $derived(
		loading ? "Assessment pending" : assessment ? healthSourceSummary(assessment) : "Backend contract missing",
	);
	const variant = $derived(state === "degraded" ? "destructive" : "outline");
	const tone = $derived(
		state === "healthy"
			? "border-emerald-500/50 text-emerald-700 dark:text-emerald-300"
			: state === "needsAttention"
				? "border-amber-500/50 text-amber-700 dark:text-amber-300"
				: "",
	);

	function raw<Value>(value: Value): string {
		return isString(value) ? value : (JSON.stringify(value) ?? String(value));
	}

	function isString<Value>(value: Value): value is Value & string {
		return String(value) === value;
	}
</script>

<div class="flex min-w-0 flex-wrap items-center gap-1.5">
	<Badge {variant} class={tone}>{label}</Badge>
	<span class="truncate text-[0.6875rem] text-muted-foreground" title={source}>Source: {source}</span>
	{#if assessment?.completeness === "partial"}
		<Badge variant="outline" class="border-dashed">Partial</Badge>
	{/if}
</div>

{#if details && assessment}
	<details class="mt-2 rounded-md border bg-background/40 p-2 text-xs">
		<summary class="cursor-pointer font-medium">Health evidence</summary>
		<ul class="mt-2 grid gap-1.5 text-muted-foreground">
			{#each assessment.evidence as evidence}
				<li>
					<span class="font-medium text-foreground">{healthSourceLabel(evidence.source)}</span>:
					{raw(evidence.raw)} — {evidence.reason}{evidence.current ? "" : " (historical)"}
				</li>
			{/each}
		</ul>
	</details>
{/if}
