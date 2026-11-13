<script lang="ts">
	import { Badge } from "@/components/ui/svelte";
	import type { ResourceSummary } from "@/lib/types";
	import type { ProblemTone } from "./model";
	import { STATUS_BADGE, TONE_LABEL } from "./view";

	let { resource, tone }: { resource: ResourceSummary; tone: ProblemTone } = $props();
</script>

<Badge variant="outline" class={STATUS_BADGE[tone]}>{resource.status ?? TONE_LABEL[tone]}</Badge>
{#if resource.healthAssessment?.reasons[0]}
	<p class="mt-1 text-muted-foreground">{resource.healthAssessment.reasons[0]}</p>
{/if}
