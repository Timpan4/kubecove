<script lang="ts">
	import { Badge, Button, Textarea } from "@/components/ui/svelte";
	import type { RbacInspectionSummary } from "@/lib/types";
	import type { RbacCockpitItem } from "./cockpitModel";
	import RbacReviewBadge from "./RbacReviewBadge.svelte";
	import {
		buildRbacEvidenceFingerprint,
		isReviewableRbacCategory,
		removeRbacReviewRecord,
		rbacReviewSubjectLabels,
		reviewStatus,
		upsertRbacReviewRecord,
		type RbacReviewDisposition,
		type RbacReviewRecord,
	} from "./reviewModel";

	let {
		inspection,
		entry,
		records,
		onRecordsChange,
	}: {
		inspection: RbacInspectionSummary;
		entry: RbacCockpitItem;
		records: RbacReviewRecord[];
		onRecordsChange: (records: RbacReviewRecord[]) => void;
	} = $props();

	let note = $state("");
	let error = $state("");
	let editorKey = $state("");
	const record = $derived(
		records.find(
			(candidate) =>
				candidate.clusterContext === inspection.cluster &&
				candidate.objectKey === entry.key,
		),
	);
	const status = $derived(reviewStatus(record, inspection, entry));
	const subjects = $derived(rbacReviewSubjectLabels(inspection, entry));

	$effect(() => {
		const nextKey = `${inspection.cluster}:${entry.key}:${record?.reviewedAt ?? "none"}`;
		if (editorKey === nextKey) return;
		editorKey = nextKey;
		note = record?.note ?? "";
		error = "";
	});

	function save(disposition: RbacReviewDisposition) {
		const evidenceFingerprint = buildRbacEvidenceFingerprint(inspection, entry);
		if (!evidenceFingerprint) return;
		try {
			onRecordsChange(
				upsertRbacReviewRecord(records, {
					clusterContext: inspection.cluster,
					objectKey: entry.key,
					evidenceFingerprint,
					disposition,
					note,
					reviewedAt: new Date().toISOString(),
				}),
			);
			error = "";
		} catch (caught) {
			error = caught instanceof Error ? caught.message : String(caught);
		}
	}

	function clearReview() {
		onRecordsChange(removeRbacReviewRecord(records, inspection.cluster, entry.key));
		note = "";
		error = "";
	}
</script>

{#if isReviewableRbacCategory(entry.category)}
	<section class="rounded-md border bg-card">
		<div class="flex items-center gap-2 border-b border-border px-3 py-2.5">
			<div class="min-w-0 flex-1">
				<p class="text-sm font-semibold">Reviewed context</p>
				<p class="mt-1 text-xs text-muted-foreground">Manual annotation. Risk and evidence stay unchanged.</p>
			</div>
			<RbacReviewBadge {inspection} {entry} {records} />
		</div>
		<div class="space-y-2.5 p-3">
			<div>
				<p class="text-[0.625rem] font-semibold uppercase tracking-wide text-muted-foreground">Affected identities</p>
				<div class="mt-1.5 flex flex-wrap gap-1">
					{#each subjects as subject}
						<Badge variant="outline" class="font-mono font-normal">{subject}</Badge>
					{:else}
						<span class="text-xs text-muted-foreground">No subjects found in loaded binding evidence.</span>
					{/each}
				</div>
			</div>
			{#if status === "stale"}
				<p class="rounded-sm border border-amber-500/35 bg-amber-500/10 p-2 text-xs text-amber-700 dark:text-amber-200">
					Evidence changed after this review. Re-review before treating its disposition as active.
				</p>
			{/if}
			<Textarea
				bind:value={note}
				aria-label="RBAC review note"
				aria-invalid={error ? "true" : undefined}
				aria-describedby={error ? "rbac-review-note-error" : undefined}
				placeholder="Why is this access expected or anomalous?"
				rows={3}
			/>
			{#if error}<p id="rbac-review-note-error" role="alert" class="text-xs text-destructive">{error}</p>{/if}
			<div class="flex flex-wrap gap-2">
				<Button size="sm" variant="outline" onclick={() => save("expected")}>Mark expected</Button>
				<Button size="sm" variant="destructive" onclick={() => save("anomalous")}>Mark anomalous</Button>
				{#if record}<Button size="sm" variant="ghost" onclick={clearReview}>Clear review</Button>{/if}
			</div>
			{#if record}
				<p class="text-xs text-muted-foreground">Reviewed {new Date(record.reviewedAt).toLocaleString()}.</p>
			{/if}
		</div>
	</section>
{/if}
