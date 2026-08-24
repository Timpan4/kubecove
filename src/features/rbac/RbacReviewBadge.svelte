<script lang="ts">
	import { Badge } from "@/components/ui/svelte";
	import type { RbacInspectionSummary } from "@/lib/types";
	import type { RbacCockpitItem } from "./cockpitModel";
	import {
		isReviewableRbacCategory,
		reviewStatus,
		type RbacReviewRecord,
	} from "./reviewModel";

	let {
		inspection,
		entry,
		records,
	}: {
		inspection: RbacInspectionSummary;
		entry: RbacCockpitItem;
		records: RbacReviewRecord[];
	} = $props();

	const record = $derived(
		records.find(
			(candidate) =>
				candidate.clusterContext === inspection.cluster &&
				candidate.objectKey === entry.key,
		),
	);
	const status = $derived(reviewStatus(record, inspection, entry));
	const label = $derived(
		status === "stale"
			? "Review stale"
			: status === "active"
				? record?.disposition === "expected"
					? "Expected"
					: "Anomalous"
				: "Unreviewed",
	);
	const badgeClass = $derived(
		status === "stale"
			? "border-amber-500/35 bg-amber-500/10 text-amber-700 dark:text-amber-200"
			: record?.disposition === "expected" && status === "active"
				? "border-emerald-500/35 bg-emerald-500/10 text-emerald-700 dark:text-emerald-200"
				: record?.disposition === "anomalous" && status === "active"
					? "border-destructive/35 bg-destructive/10 text-destructive"
					: "text-muted-foreground",
	);
</script>

{#if isReviewableRbacCategory(entry.category)}
	<Badge variant="outline" class={badgeClass}>{label}</Badge>
{/if}
