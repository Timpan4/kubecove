<script lang="ts">
	import { GitCommitHorizontal } from "lucide-svelte";
	import TimestampText from "@/components/TimestampText.svelte";
	import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/svelte";
	import type { ArgoApplicationHistory, ArgoApplicationRef, ArgoApplicationSummary, JsonObject } from "@/lib/types";
	import { argoSyncPolicyLabel } from "./argo-application-yaml";
	import { argoHistoryKey } from "./argo-workspace-model";
	import { argoHistoryRevision as historyRevision, argoStringAt as stringAt } from "./argo-application-display";

	let { matchingSummary, applicationSpec, configuredRevision, history, application, selectedHistoryKey, selectedHistory, selectHistory }: {
		matchingSummary: ArgoApplicationSummary | null;
		applicationSpec: JsonObject | null;
		configuredRevision: string;
		history: ArgoApplicationHistory[];
		application: ArgoApplicationRef;
		selectedHistoryKey: string | null;
		selectedHistory: ArgoApplicationHistory | null;
		selectHistory: (key: string) => void;
	} = $props();

	const primarySource = $derived(matchingSummary?.sources?.[0] ?? null);
	const repository = $derived(
		primarySource?.repoUrl ??
			matchingSummary?.sourceRepo ??
			stringAt(applicationSpec, "source", "repoURL") ??
			stringAt(applicationSpec, "sources", "0", "repoURL") ??
			"Repository unavailable",
	);
	const sourcePath = $derived(
		primarySource?.path ??
			primarySource?.chart ??
			stringAt(applicationSpec, "source", "path") ??
			stringAt(applicationSpec, "source", "chart") ??
			stringAt(applicationSpec, "sources", "0", "path") ??
			"Path unavailable",
	);
	const destination = $derived(
		matchingSummary?.destinationNamespace ??
			stringAt(applicationSpec, "destination", "namespace") ??
			matchingSummary?.destinationServer ??
			stringAt(applicationSpec, "destination", "server") ??
			"Destination unavailable",
	);
	const policy = $derived(argoSyncPolicyLabel(applicationSpec));
</script>

<section class="grid gap-3 @min-[36rem]:grid-cols-[minmax(0,1.35fr)_minmax(12rem,0.65fr)]">
	<div class="min-w-0 rounded-lg border bg-surface-1 p-3">
		<div class="text-xs font-semibold uppercase tracking-wide text-muted-foreground">Delivery</div>
		<div class="mt-3 grid min-w-0 grid-cols-[4.5rem_minmax(0,1fr)] gap-x-2 gap-y-2 text-xs">
			<span class="text-muted-foreground">Repository</span><span class="min-w-0 break-all">{repository}</span>
			<span class="text-muted-foreground">Path</span><span class="min-w-0 break-all">{sourcePath}</span>
			<span class="text-muted-foreground">Revision</span><span class="min-w-0 break-all">{configuredRevision}</span>
			<span class="text-muted-foreground">Destination</span><span class="min-w-0 break-all">{destination}</span>
			<span class="text-muted-foreground">Policy</span><span>{policy}</span>
		</div>
	</div>
	<div class="min-w-0 rounded-lg border bg-surface-1 p-3">
		<div class="flex items-center gap-2 text-xs font-semibold uppercase tracking-wide text-muted-foreground"><GitCommitHorizontal class="size-3.5" /> Recent deployment</div>
		{#if history.length}
			{#if history.length > 1}
				<Select value={selectedHistoryKey ?? ""} onValueChange={selectHistory}>
					<SelectTrigger class="mt-3 w-full" ariaLabel="Recent deployment"><SelectValue>{selectedHistory ? historyRevision(selectedHistory) : "Select deployment"}</SelectValue></SelectTrigger>
					<SelectContent><SelectGroup>{#each history as entry}<SelectItem value={argoHistoryKey(application, entry)} label={historyRevision(entry)}>{historyRevision(entry)}</SelectItem>{/each}</SelectGroup></SelectContent>
				</Select>
			{/if}
			{#if selectedHistory}<div class="mt-3 break-all text-sm font-semibold">{historyRevision(selectedHistory)}</div><div class="mt-1 text-xs leading-relaxed text-muted-foreground"><TimestampText value={selectedHistory.deployedAt} fallback="No deployment timestamp" precision="second" /> by {selectedHistory.initiatedBy ?? "unknown"}</div>{/if}
		{:else}
			<div class="mt-3 text-xs text-muted-foreground">No deployment history reported.</div>
		{/if}
	</div>
</section>
