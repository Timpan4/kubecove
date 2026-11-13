<script lang="ts">
	import { createQuery } from "@tanstack/svelte-query";
	import { AlertTriangle, Boxes } from "lucide-svelte";
	import FriendlyError from "@/components/FriendlyError.svelte";
	import ResourceScopeWarnings from "@/features/resources/ResourceScopeWarnings.svelte";
	import { Alert, AlertDescription, AlertTitle, Badge, Button, Card, CardContent, CardDescription, CardHeader, CardTitle, Spinner } from "@/components/ui/svelte";
	import { buildWorkspaceFetchKeys, buildWorkspaceFetchPlans, fetchWorkspaceResources, WorkspaceResourceLoadError } from "./query";
	import { kindLabel } from "@/lib/kind-identity";
	import { loadingDelay } from "@/lib/loading-delay.svelte";
	import { cancelledQueryRetry } from "@/lib/query-retry";
	import { queryKeys } from "@/lib/queryKeys";
	import { createTauriClient, listKubeContexts, listNamespaces, listResourceKinds } from "@/lib/tauri";
	import type { ResourceSummary } from "@/lib/types";
	import { normalizeEntryPoints, resourceFromEntryPoint, type WorkspaceEntryPoint } from "@/lib/workspace-entry-points";
	import { buildWorkspaceCompareEntries, buildWorkspaceCompareSummaries, buildWorkspaceHealthSummary, computeRestoreStatus, workspaceClusterGroupLabel, workspaceScopeContexts, type SavedWorkspace, type WorkspaceShortcut } from "@/lib/workspaces";
	import type { HealthFilter } from "@/features/resources/helpers";
	import type { IncidentFilter } from "@/features/incidents";
	import type { WorkspaceReadContext } from "@/lib/workspaceReadContext";
	import OverviewLayout from "./overview/OverviewLayout.svelte";
	import OverviewGitOps from "./overview/OverviewGitOps.svelte";
	import { buildOverviewModel } from "./overview/model";
	import { entryRows } from "./overview/entries";
	let {
		workspace,
		workspaceReadContext,
		onOpenResources,
		onOpenResource,
		onReconcileEntryPoints,
		onOpenArgo,
		onOpenIncidents,
		onOpenPortForwards,
	}: {
		workspace: SavedWorkspace;
		workspaceReadContext: WorkspaceReadContext;
		onOpenResources: (
			namespace?: string,
			initialSearch?: string,
			initialGitOpsFilter?: string,
			initialHealthFilter?: HealthFilter,
		) => void;
		onOpenResource: (resource: ResourceSummary) => void;
		onReconcileEntryPoints: (
			resources: ResourceSummary[],
			coverage: ReturnType<typeof buildWorkspaceFetchPlans>,
		) => void;
		onOpenArgo: (argoApp?: string, namespace?: string) => void;
		onOpenIncidents: (filter?: IncidentFilter) => void;
		onOpenPortForwards: () => void;
		onOpenLauncher: () => void;
	} = $props();

	const client = createTauriClient();

	const sourceReady = $derived(workspaceReadContext.sourceReady);
	const kubeconfigSourceKey = $derived(workspaceReadContext.kubeconfigSourceKey);
	const workspaceContextKey = $derived(workspaceScopeContexts(workspace.scope).join("|"));

	const contextsQuery = createQuery(() => ({
		queryKey: queryKeys.kubeContexts(kubeconfigSourceKey),
		queryFn: () => listKubeContexts(client, kubeconfigSourceKey),
		enabled: sourceReady,
	}));
	const contexts = $derived(contextsQuery.data ?? []);
	const clusterAvailable = $derived(
		contextsQuery.isPending ||
			contexts.some((context) => context.name === workspace.scope.clusterContext),
	);

	const namespacesQuery = createQuery(() => ({
		queryKey: queryKeys.namespaces(workspace.scope.clusterContext, kubeconfigSourceKey),
		queryFn: () => listNamespaces(client, workspace.scope.clusterContext, kubeconfigSourceKey),
		enabled: sourceReady && clusterAvailable && !contextsQuery.isPending,
		retry: false,
	}));
	const namespaces = $derived((namespacesQuery.data ?? []).map((namespace) => namespace.name));

	const kindsQuery = createQuery(() => ({
		queryKey: queryKeys.resourceKinds(workspace.scope.clusterContext, kubeconfigSourceKey),
		queryFn: () => listResourceKinds(client, workspace.scope.clusterContext, kubeconfigSourceKey),
		enabled: sourceReady && clusterAvailable && !contextsQuery.isPending,
		retry: cancelledQueryRetry,
	}));
	const discoveredKinds = $derived(kindsQuery.data ?? []);

	const restoreStatus = $derived(
		computeRestoreStatus(workspace, contexts, namespaces, discoveredKinds),
	);
	const availableNamespaces = $derived(
		workspace.scope.namespaces.filter(
			(namespace) => !restoreStatus.missingNamespaces.includes(namespace),
		),
	);
	const workspaceFetchKeys = $derived(
		buildWorkspaceFetchKeys(workspace.scope, availableNamespaces),
	);
	const resourceQueryKey = $derived(
		queryKeys.resources(workspaceContextKey, workspaceFetchKeys, kubeconfigSourceKey),
	);
	const resourceQueryIdentity = $derived(JSON.stringify(resourceQueryKey));

	const resourcesQuery = createQuery(() => ({
		queryKey: resourceQueryKey,
		queryFn: () => fetchWorkspaceResources(workspace.scope, availableNamespaces, kubeconfigSourceKey),
		enabled:
			sourceReady &&
			restoreStatus.clusterAvailable &&
			!namespacesQuery.isPending &&
			!kindsQuery.isPending,
		staleTime: 30_000,
	}));
	let successfulResourceRetryKey = $state<string | null>(null);
	const failedResourceTarget = $derived(
		resourcesQuery.error instanceof WorkspaceResourceLoadError
			? resourcesQuery.error.clusterContexts
					.map((context) => `context "${context}"`)
					.join(", ")
			: `context "${workspace.scope.clusterContext}"`,
	);
	async function retryWorkspaceResources() {
		const retryKey = resourceQueryIdentity;
		successfulResourceRetryKey = null;
		const result = await resourcesQuery.refetch();
		if (result.isSuccess && resourceQueryIdentity === retryKey) {
			successfulResourceRetryKey = retryKey;
		}
	}
	const rows = $derived(resourcesQuery.data?.resources ?? []);
	const entryPoints = $derived(normalizeEntryPoints(workspace.entryPoints));
	$effect(() => {
		if (resourcesQuery.isSuccess && !resourcesQuery.isPlaceholderData) {
			onReconcileEntryPoints(rows, resourcesQuery.data?.coverage ?? []);
		}
	});
	const health = $derived(buildWorkspaceHealthSummary(rows));

	const hasRestoreWarning = $derived(
		contextsQuery.isSuccess &&
			(!restoreStatus.clusterAvailable ||
				restoreStatus.missingClusterContexts.length > 0 ||
				restoreStatus.missingNamespaces.length > 0 ||
				restoreStatus.missingKinds.length > 0),
	);
	const compareSummaries = $derived(
		buildWorkspaceCompareSummaries(buildWorkspaceCompareEntries(workspace.scope).slice(0, 2), rows),
	);

	function openShortcut(shortcut: WorkspaceShortcut) {
		if (shortcut.kind === "argo") {
			onOpenArgo(shortcut.argoApp);
			return;
		}
		onOpenResources(shortcut.namespace);
	}

	function openEntryPoint(entry: WorkspaceEntryPoint) {
		if (entry.kind === "namespace") {
			onOpenResources(entry.namespace ?? entry.name);
			return;
		}
		if (entry.kind === "app") {
			onOpenArgo(entry.name, entry.namespace);
			return;
		}
		const resource = resourceFromEntryPoint(entry);
		if (resource) onOpenResource(resource);
	}


	const healthLoading = loadingDelay(() => resourcesQuery.isPending);
	const model = $derived(buildOverviewModel(rows));
	const view = $derived({
		model, health, hasData: resourcesQuery.data !== undefined,
		partial: resourcesQuery.isError || hasRestoreWarning || (resourcesQuery.data?.failures.length ?? 0) > 0,
		multiContext: workspaceScopeContexts(workspace.scope).length > 1, primaryContext: workspace.scope.clusterContext,
		pinned: entryRows(entryPoints.pinned, rows, model), recent: entryRows(entryPoints.recent, rows, model),
		shortcuts: workspace.shortcuts, portForwardCount: workspace.portForwards.length,
		loadState: LoadState, gitOps: GitOps,
		onOpenResources: (namespace?: string, filter?: HealthFilter) => onOpenResources(namespace, undefined, undefined, filter),
		onOpenResource, onOpenEntry: openEntryPoint, onOpenShortcut: openShortcut, onOpenIncidents,
		onOpenApp: (resource: ResourceSummary) => {
			const owner = resource.gitOpsOwner;
			if (owner?.provider === "argo" && owner.kind === "Application" && owner.namespace) {
				onOpenResource({ kind: "Application", name: owner.name, namespace: owner.namespace, cluster: resource.cluster,
					apiVersion: "argoproj.io/v1alpha1", group: "argoproj.io", version: "v1alpha1", plural: "applications",
					namespaced: true, dynamic: true, age: "", health: "unknown", healthAssessment: null });
			}
		}, onOpenPortForwards,
	});
</script>

<section class="@container mx-auto flex w-full max-w-7xl flex-col gap-4 p-4 md:p-6">
	<header class="flex flex-wrap items-end justify-between gap-3 border-b pb-3">
		<div class="min-w-0">
		<p class="text-[0.6875rem] font-semibold uppercase tracking-wide text-muted-foreground">
			Workspace Overview
		</p>
		<h1 class="mt-1 truncate text-lg font-semibold tracking-tight">{workspace.name}</h1>
		<div class="mt-2 flex flex-wrap gap-2">
			<Badge variant="outline" class="rounded-sm">{workspaceClusterGroupLabel(workspace.scope)}</Badge>
			<Badge variant="outline" class="rounded-sm">
				{workspace.scope.namespaces.length || "All"} namespaces
			</Badge>
			<Badge variant="outline" class="max-w-96 rounded-sm">
				<span class="truncate">{workspace.scope.kinds.map(kindLabel).join(", ")}</span>
			</Badge>
		</div>
			</div>
		<Button variant="outline" onclick={() => onOpenResources()}><Boxes /> Open resources</Button>
	</header>

	{#if hasRestoreWarning}
		<Alert class="border-amber-500/40 text-amber-200">
			<AlertTriangle class="size-3.5" />
			<AlertTitle>Unavailable saved scope</AlertTitle>
			<AlertDescription class="text-amber-200/90">
				{#if !restoreStatus.clusterAvailable}<div>Context missing: {workspace.scope.clusterContext}</div>{/if}
				{#if restoreStatus.missingClusterContexts.length > 0}
					<div>Group members: {restoreStatus.missingClusterContexts.join(", ")}</div>
				{/if}
				{#if restoreStatus.missingNamespaces.length > 0}
					<div>Namespaces: {restoreStatus.missingNamespaces.join(", ")}</div>
				{/if}
				{#if restoreStatus.missingKinds.length > 0}
					<div>Kinds: {restoreStatus.missingKinds.join(", ")}</div>
				{/if}
			</AlertDescription>
		</Alert>
	{/if}

	<OverviewLayout {view} />
	{#if compareSummaries.length > 0}
		<Card>
			<CardHeader>
				<CardTitle>Compare</CardTitle>
				<CardDescription>Health split for saved workspace compare shortcuts.</CardDescription>
			</CardHeader>
			<CardContent class="grid gap-3 md:grid-cols-2">
				{#each compareSummaries as summary}
					<div class="rounded-md border bg-surface-1 p-3">
						<div class="text-sm font-medium">{summary.entry.label}</div>
						<div class="mt-2 grid grid-cols-2 gap-2 text-xs">
							{@render CompareSide(summary.entry.leftLabel, summary.left)}
							{@render CompareSide(summary.entry.rightLabel, summary.right)}
						</div>
					</div>
				{/each}
			</CardContent>
		</Card>
	{/if}
</section>
{#snippet LoadState()}
	{#if healthLoading.active}
		<div class="flex min-h-16 items-center justify-center gap-2 text-sm text-muted-foreground">
			{#if healthLoading.shown}<Spinner class="size-4" /> Loading workspace health...{/if}
		</div>
	{:else if resourcesQuery.isError}
		<div class="grid gap-2">
			<FriendlyError
				error={resourcesQuery.error}
				context={{
					operation: "resourcesLoad",
					fallbackTitle: "Failed to refresh workspace resources",
					target: failedResourceTarget,
					partial: true,
				}}
			/>
			<Button
				type="button"
				variant="outline"
				size="sm"
				disabled={resourcesQuery.isFetching}
				onclick={() => void retryWorkspaceResources()}
			>
				{resourcesQuery.isFetching ? "Retrying workspace resources..." : "Retry workspace resources"}
			</Button>
		</div>
	{:else if successfulResourceRetryKey === resourceQueryIdentity}
		<p role="status" class="text-xs text-emerald-300">
			Workspace resources loaded successfully after retry.
		</p>
	{/if}
	{#if resourcesQuery.isSuccess}
		<ResourceScopeWarnings failures={resourcesQuery.data?.failures ?? []} />
	{/if}
{/snippet}


{#snippet GitOps()}
	<OverviewGitOps clusterContext={workspace.scope.clusterContext} {kubeconfigSourceKey}
		enabled={sourceReady && restoreStatus.clusterAvailable && !namespacesQuery.isPending} {onOpenArgo} {onOpenResource} />
{/snippet}
{#snippet CompareSide(
	label: string,
	health: {
		total: number;
		healthy: number;
		attention: number;
		degraded: number;
		unknown: number;
		notEvaluated: number;
		restartEvidence: number;
	},
)}
	<div class="rounded border bg-background p-2">
		<div class="truncate font-medium">{label}</div>
		<div class="mt-1 text-muted-foreground">
			{health.total} total / {health.healthy} healthy / {health.degraded} degraded / {health.attention} need attention
		</div>
	</div>
{/snippet}
