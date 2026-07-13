<script lang="ts">
	import { createQuery } from "@tanstack/svelte-query";
	import {
		Select,
		SelectContent,
		SelectGroup,
		SelectItem,
		SelectTrigger,
		SelectValue,
		TabsContent,
	} from "@/components/ui/svelte";
	import { listDeploymentRevisions, type TauriClient } from "@/lib/tauri";
	import { queryKeys } from "@/lib/queryKeys";
	import type { DeploymentRevision, ResourceSummary } from "@/lib/types";
	import {
		deploymentRevisionViewState,
		reconcileRevisionSelection,
		revisionLabel,
	} from "./deployment-revisions-model";
	import { buildCompactUnifiedDiff, diffLineClassName } from "./yamlTabDiff";

	let {
		client,
		resource,
		kubeconfigSourceKey,
		active,
	}: {
		client: TauriClient;
		resource: ResourceSummary;
		kubeconfigSourceKey?: string;
		active: boolean;
	} = $props();

	const revisionsQuery = createQuery<DeploymentRevision[]>(() => ({
		queryKey: queryKeys.deploymentRevisions(resource, kubeconfigSourceKey),
		queryFn: () =>
			listDeploymentRevisions(
				client,
				resource.cluster,
				resource.name,
				resource.namespace!,
				kubeconfigSourceKey,
			),
		retry: false,
		staleTime: 30_000,
		enabled: active,
	}));

	let selectedRevisionName = $state<string | null>(null);
	let comparisonRevisionName = $state<string | null>(null);
	const revisions = $derived(revisionsQuery.data ?? []);
	const viewState = $derived(
		deploymentRevisionViewState(revisionsQuery.isPending, revisionsQuery.isError, revisions),
	);
	const selectedRevision = $derived(
		revisions.find((revision) => revision.name === selectedRevisionName),
	);
	const comparisonRevision = $derived(
		revisions.find((revision) => revision.name === comparisonRevisionName),
	);
	const comparisonOptions = $derived(
		revisions.filter((revision) => revision.name !== selectedRevisionName),
	);
	const diffLines = $derived(
		selectedRevision && comparisonRevision
			? buildCompactUnifiedDiff(comparisonRevision.podTemplateYaml, selectedRevision.podTemplateYaml)
			: [],
	);

	$effect(() => {
		const selection = reconcileRevisionSelection(
			revisions,
			selectedRevisionName,
			comparisonRevisionName,
		);
		selectedRevisionName = selection.selectedName;
		comparisonRevisionName = selection.comparisonName;
	});
</script>

<TabsContent value="revisions" class="min-h-0 overflow-auto">
	{#if viewState === "loading"}
		<p class="p-4 text-sm text-muted-foreground">Loading Deployment revisions…</p>
	{:else if viewState === "error"}
		<p class="p-4 text-sm text-destructive">Unable to load Deployment revisions.</p>
	{:else if viewState === "empty"}
		<p class="p-4 text-sm text-muted-foreground">No controller-owned ReplicaSet revisions were found.</p>
	{:else}
		<div class="grid min-h-0 gap-3 p-1 lg:grid-cols-[15rem_minmax(0,1fr)]">
			<div class="space-y-2">
				{#each revisions as revision (revision.name)}
					<button
						type="button"
						class="w-full rounded-md border p-3 text-left text-sm hover:bg-muted/50 {selectedRevision?.name === revision.name ? 'border-primary bg-primary/5' : 'border-border'}"
						onclick={() => (selectedRevisionName = revision.name)}
					>
						<div class="font-medium">{revisionLabel(revision)}</div>
						<div class="mt-1 truncate text-xs text-muted-foreground">
							{revision.changeCause ?? revision.name}
						</div>
					</button>
				{/each}
			</div>
			<div class="min-w-0 rounded-md border">
				{#if viewState === "single"}
					<p class="p-4 text-sm text-muted-foreground">
						Only one revision is available; there is no other pod template to compare.
					</p>
				{:else if selectedRevision && comparisonRevision}
					<div class="flex flex-wrap items-center gap-2 border-b px-3 py-2 text-sm">
						<span class="font-medium">{revisionLabel(selectedRevision)}</span>
						<span class="text-muted-foreground">compared with</span>
						<Select
							value={comparisonRevisionName ?? undefined}
							items={comparisonOptions.map((revision) => ({
								value: revision.name,
								label: revisionLabel(revision),
							}))}
							onValueChange={(value: string) => (comparisonRevisionName = value)}
						>
							<SelectTrigger class="h-8 w-44" aria-label="Comparison revision">
								<SelectValue placeholder="Select revision" />
							</SelectTrigger>
							<SelectContent>
								<SelectGroup>
									{#each comparisonOptions as revision (revision.name)}
										<SelectItem value={revision.name}>{revisionLabel(revision)}</SelectItem>
									{/each}
								</SelectGroup>
							</SelectContent>
						</Select>
					</div>
					<pre class="overflow-auto p-3 text-xs leading-5">{#each diffLines as line}<span class={diffLineClassName(line.type)}>{line.text}{"\n"}</span>{/each}</pre>
				{/if}
			</div>
		</div>
	{/if}
</TabsContent>
