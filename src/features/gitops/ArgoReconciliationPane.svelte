<script lang="ts">
	import { createQuery } from "@tanstack/svelte-query";
	import { GitCompareArrows } from "lucide-svelte";
	import type { Snippet } from "svelte";
	import YamlCodeEditor from "@/components/YamlCodeEditor.svelte";
	import { Badge, Button, Empty, EmptyDescription, EmptyHeader, EmptyTitle, SegmentedControl, Spinner } from "@/components/ui/svelte";
	import { finiteRead } from "@/lib/finite-read-lifecycle";
	import { loadingDelay } from "@/lib/loading-delay.svelte";
	import { queryKeys } from "@/lib/queryKeys";
	import { settingsStore } from "@/lib/settings-store";
	import { getArgoResourceComparison, type TauriClient } from "@/lib/tauri";
	import type { ArgoApplicationRef, ArgoManagedResource } from "@/lib/types";
	import type { argoApplicationInspection } from "@/lib/argo-application-inspection.svelte";
	import ArgoResourceDiff from "./ArgoResourceDiff.svelte";
	import { projectArgoResourceComparison, resolveArgoResourceComparison } from "./argo-resource-display";
	import { argoComparisonIndex, argoReconciliationResources, argoResourceIdentityKey, preserveArgoResourceSelection } from "./argo-workspace-model";

	type Inspection = ReturnType<typeof argoApplicationInspection>["current"];
	type DiffView = "changes" | "target" | "live";
	let {
		client, applicationRequest, scopeKey, clusterContext, workspaceId, kubeconfigEnvVar,
		managedResources, snapshot, request, sourceReady, connectionReady, transport, connectionId,
		active, loading, dataError, delivery,
	}: {
		client: TauriClient;
		applicationRequest: ArgoApplicationRef;
		scopeKey: string;
		clusterContext: string;
		workspaceId: string;
		kubeconfigEnvVar: string | undefined;
		managedResources: ArgoManagedResource[];
		snapshot: Inspection["data"];
		request: Inspection["request"];
		sourceReady: boolean;
		connectionReady: boolean;
		transport: "connected" | "kubernetes";
		connectionId: string;
		active: boolean;
		loading: boolean;
		dataError: unknown;
		delivery: Snippet;
	} = $props();
	const loadingIndicator = loadingDelay(() => loading);
	const redactSecrets = $derived($settingsStore.redactSecrets);
	let selectedResource = $state.raw<ArgoManagedResource | null>(null);
	let diffView = $state<DiffView>("changes");
	let expandedRemovalKeys = $state<string[]>([]);
	let appliedScopeKey = "";
	$effect(() => {
		const next = scopeKey;
		if (appliedScopeKey && appliedScopeKey !== next) {
			selectedResource = null;
			diffView = "changes";
			expandedRemovalKeys = [];
		}
		appliedScopeKey = next;
	});

	const diffViewOptions: { value: DiffView; label: string }[] = [
		{ value: "changes", label: "Changes" },
		{ value: "target", label: "Desired" },
		{ value: "live", label: "Live" },
	];

	const reconciliationResources = $derived(argoReconciliationResources(managedResources));
	const comparisonsByKey = $derived(argoComparisonIndex(snapshot?.comparisons ?? []));
	const selectedResourceKey = $derived(
		selectedResource ? argoResourceIdentityKey(selectedResource) : null,
	);
	const lazyComparisonEnabled = $derived(
		Boolean(selectedResourceKey) &&
			active &&
			sourceReady &&
			connectionReady &&
			transport === "connected" &&
			snapshot?.transport === "connected",
	);
	const selectedComparisonQueryKey = $derived(
		queryKeys.argoWorkspaceSelectedResource(
			clusterContext,
			workspaceId,
			applicationRequest.name,
			applicationRequest.namespace,
			applicationRequest.uid,
			redactSecrets,
			transport,
			connectionId,
			selectedResource?.group,
			selectedResource?.kind,
			selectedResource?.namespace,
			selectedResource?.name,
			kubeconfigEnvVar,
		),
	);
	const selectedComparison = createQuery(() => ({
		...finiteRead({
			queryKey: selectedComparisonQueryKey,
			label: "argo.resource-comparison",
			read: async (cancellable) => {
				if (!selectedResource || !request) throw new Error("No selected resource");
				return getArgoResourceComparison(
					client,
					{ ...request, resource: selectedResource },
					cancellable,
				);
			},
		}),
		enabled: lazyComparisonEnabled,
		staleTime: 30_000,
		refetchInterval: lazyComparisonEnabled ? 15_000 : false,
		refetchIntervalInBackground: false,
		retry: false,
		gcTime: redactSecrets ? undefined : 0,
	}));

	const snapshotFreshness = $derived(
		snapshot?.connectedFallback
			? "Connected Argo CD is unavailable; Kubernetes refreshes this fallback snapshot every 15 seconds while visible."
			: snapshot?.transport === "connected"
				? "Connected Argo CD refreshes this snapshot every 15 seconds while visible."
				: "Kubernetes refreshes this snapshot every 15 seconds while visible.",
	);
	const diffResources = $derived(selectedResource ? [selectedResource] : reconciliationResources);
	const comparisonRows = $derived(loadingIndicator.active || dataError ? [] : diffResources.map((item) => {
		const comparison = resolveArgoResourceComparison(item, comparisonsByKey, snapshot?.transport, selectedResourceKey, selectedComparison.data);
		return { item, documents: projectArgoResourceComparison(item, comparison) };
	}));
	$effect(() => {
		const next = preserveArgoResourceSelection(selectedResource, reconciliationResources);
		if (next !== selectedResource) selectedResource = next;
	});
	function selectResource(item: ArgoManagedResource) {
		selectedResource = item;
	}

	function showAllChanges() {
		selectedResource = null;
	}

	function resourceSelected(item: ArgoManagedResource): boolean {
		if (!selectedResource) return false;
		const selectedKey = argoResourceIdentityKey(selectedResource);
		return selectedKey !== null && selectedKey === argoResourceIdentityKey(item);
	}

	function removalExpanded(item: ArgoManagedResource): boolean {
		const key = argoResourceIdentityKey(item);
		return key !== null && expandedRemovalKeys.includes(key);
	}

	function toggleRemoval(item: ArgoManagedResource) {
		const key = argoResourceIdentityKey(item);
		if (!key) return;
		expandedRemovalKeys = expandedRemovalKeys.includes(key)
			? expandedRemovalKeys.filter((itemKey) => itemKey !== key)
			: [...expandedRemovalKeys, key];
	}

	function resourceLabel(item: ArgoManagedResource): string {
		return `${item.kind ?? "Resource"} ${item.namespace ? `${item.namespace}/` : ""}${item.name ?? "unknown"}`;
	}

</script>

<section class="rounded-lg border bg-surface-1 p-3">
	<div class="flex items-center justify-between gap-2"><div><div class="text-xs font-semibold uppercase tracking-wide text-muted-foreground">Change navigator</div><div class="mt-0.5 text-xs text-muted-foreground">Focus one reconciliation item or keep the complete snapshot.</div></div><Badge variant="outline">{reconciliationResources.length}</Badge></div>
	<div class="mt-3 flex gap-2 overflow-x-auto pb-1">
		<Button type="button" size="sm" variant={selectedResource ? "outline" : "secondary"} class="shrink-0" aria-pressed={!selectedResource} onclick={showAllChanges}>All changes</Button>
		{#each reconciliationResources as item (argoResourceIdentityKey(item) ?? resourceLabel(item))}
			<Button type="button" size="sm" variant={resourceSelected(item) ? "secondary" : "outline"} class="h-auto min-w-36 max-w-56 shrink-0 justify-start p-2 text-left" aria-pressed={resourceSelected(item)} onclick={() => selectResource(item)}>
				<span class="min-w-0"><span class="block truncate text-xs font-medium">{item.kind ?? "Resource"} · {item.name ?? "unknown"}</span><span class="mt-0.5 flex flex-wrap gap-1"><Badge variant="outline">{item.status ?? "Unknown"}</Badge>{#if item.requiresPruning}<Badge variant="destructive">Prune</Badge>{/if}</span></span>
			</Button>
		{/each}
	</div>
</section>


{@render delivery()}

<section class="min-w-0 rounded-lg border bg-surface-1 p-3">
	<div class="flex flex-wrap items-start justify-between gap-2">
		<div><div class="flex items-center gap-2 text-xs font-semibold uppercase tracking-wide text-muted-foreground"><GitCompareArrows class="size-3.5" /> {selectedResource ? "Focused reconciliation" : "Application snapshot"}</div><div class="mt-1 break-all text-xs text-muted-foreground">{selectedResource ? resourceLabel(selectedResource) : `${diffResources.length} reconciliation ${diffResources.length === 1 ? "item" : "items"} in this snapshot.`} {snapshotFreshness}</div></div>
		<div class="flex max-w-full flex-wrap items-center gap-2">{#if selectedResource}<Button type="button" size="xs" variant="outline" onclick={showAllChanges}>All changes</Button>{/if}<SegmentedControl value={diffView} options={diffViewOptions} onChange={(value) => (diffView = value)} ariaLabel="Reconciliation document view" size="sm" /></div>
	</div>
	<div class="mt-3 space-y-4">
		{#if loadingIndicator.active}
			{#if loadingIndicator.shown}<div class="flex items-center gap-2 rounded-lg border p-3 text-xs text-muted-foreground" role="status"><Spinner /> Loading Application snapshot</div>{/if}
		{:else if dataError}
			<Empty><EmptyHeader><EmptyTitle>Application snapshot unavailable</EmptyTitle><EmptyDescription>Retry Application data before evaluating reconciliation state.</EmptyDescription></EmptyHeader></Empty>
		{:else}
			{#each comparisonRows as { item, documents } (argoResourceIdentityKey(item) ?? resourceLabel(item))}
			{@const removal = item.requiresPruning === true}
			<article class="min-w-0 overflow-hidden rounded-lg border bg-background">
				<div class="flex flex-wrap items-center gap-2 border-b bg-surface-1 px-3 py-2">
					<div class="min-w-0 flex-1 truncate text-xs font-semibold" title={resourceLabel(item)}>{resourceLabel(item)}</div>
					{#if removal}<Badge variant="destructive">Remove on sync</Badge>{:else if documents.added}<Badge variant="outline" class="border-emerald-500/35 bg-emerald-500/10 text-emerald-700 dark:text-emerald-300">Added</Badge>{:else if documents.modified}<Badge variant="outline" class="border-amber-500/40 bg-amber-500/10 text-amber-700 dark:text-amber-300">Modified</Badge>{:else}<Badge variant="secondary">In sync</Badge>{/if}
					{#if removal}<Button type="button" size="xs" variant="ghost" aria-expanded={removalExpanded(item)} onclick={() => toggleRemoval(item)}>{removalExpanded(item) ? "Hide diff" : "Show diff"}</Button>{/if}
					{#if !selectedResource}<Button type="button" size="xs" variant="ghost" onclick={() => selectResource(item)}>Focus</Button>{/if}
				</div>
				{#if removal}
					<div class="bg-destructive/10 p-3 font-sans text-xs leading-relaxed"><div class="font-medium text-destructive">Live-only resource</div><div class="mt-1 text-muted-foreground">This resource is no longer present in desired revision. Sync with prune removes live object. Live YAML stays collapsed until Show diff is selected.</div></div>
				{/if}
				{#if removal && diffView !== "target" && !removalExpanded(item)}
					<span class="sr-only">Live YAML collapsed.</span>
				{:else if diffView === "target" && removal}
					<div class="border-t p-3 font-sans text-xs leading-relaxed"><div class="font-medium">No desired manifest</div><div class="mt-1 text-muted-foreground">Selected revision no longer contains this resource.</div></div>
				{:else if diffView === "changes" && documents.desired && documents.normalizedLive}
					<div class="overflow-x-auto border-t font-mono text-[0.6875rem] leading-5">
						<ArgoResourceDiff desired={documents.desired} live={documents.normalizedLive} />
					</div>
				{:else if diffView === "changes" && removal && documents.live}
					<div class="border-t [&_.yaml-code-editor]:rounded-none [&_.yaml-code-editor]:border-0 [&_.yaml-code-editor]:shadow-none"><YamlCodeEditor value={documents.live} editable={false} minHeight="180px" showErrorLens={false} /></div>
				{:else if diffView === "target" && documents.desired}
					<div class="border-t [&_.yaml-code-editor]:rounded-none [&_.yaml-code-editor]:border-0 [&_.yaml-code-editor]:shadow-none"><YamlCodeEditor value={documents.desired} editable={false} minHeight="180px" showErrorLens={false} /></div>
				{:else if diffView === "live" && documents.live}
					<div class="border-t [&_.yaml-code-editor]:rounded-none [&_.yaml-code-editor]:border-0 [&_.yaml-code-editor]:shadow-none"><YamlCodeEditor value={documents.live} editable={false} minHeight="180px" showErrorLens={false} /></div>
				{:else}
					<div class="border-t p-3 text-xs leading-relaxed text-muted-foreground">{documents.exact === false || transport === "kubernetes" ? "Exact target and live manifests are unavailable through Kubernetes fallback. Select a connected Argo CD profile for manifest comparison." : `No ${diffView === "target" ? "desired" : diffView === "live" ? "live" : "comparison"} manifest reported.`}</div>
				{/if}
			</article>
			{:else}
				<Empty><EmptyHeader><EmptyTitle>No pending changes</EmptyTitle><EmptyDescription>Inspector reports no out-of-sync, degraded, progressing, or removal resources.</EmptyDescription></EmptyHeader></Empty>
			{/each}
		{/if}
	</div>
</section>