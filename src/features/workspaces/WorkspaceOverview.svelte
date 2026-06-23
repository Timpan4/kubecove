<script lang="ts">
	import { createQueries, createQuery } from "@tanstack/svelte-query";
	import {
		AlertTriangle,
		Boxes,
		Cable,
		FolderOpen,
		GitBranch,
		GitCompareArrows,
		Layers,
	} from "lucide-svelte";
	import {
		Alert,
		AlertDescription,
		AlertTitle,
		Badge,
		Button,
		Card,
		CardContent,
		CardDescription,
		CardHeader,
		CardTitle,
		Empty,
		EmptyDescription,
		EmptyHeader,
		EmptyTitle,
		Separator,
		Spinner,
	} from "@/components/ui/svelte";
	import { buildWorkspaceFetchKeys, fetchWorkspaceResources } from "@/features/workspaces/query";
	import { queryKeys } from "@/lib/queryKeys";
	import {
		createTauriClient,
		detectArgoCD,
		detectFlux,
		getKubeconfigSources,
		listArgoApplications,
		listFluxResources,
		listKubeContexts,
		listNamespaces,
		listResourceKinds,
	} from "@/lib/tauri";
	import type { FluxResourceSummary } from "@/lib/types";
	import {
		buildWorkspaceCompareEntries,
		buildWorkspaceCompareSummaries,
		buildWorkspaceHealthSummary,
		computeRestoreStatus,
		resourceKindLabel,
		workspaceClusterGroupLabel,
		workspaceScopeContexts,
		type SavedWorkspace,
		type WorkspaceShortcut,
	} from "@/lib/workspaces";
	import type { HealthFilter } from "@/features/resources/helpers";
	import type { IncidentFilter } from "@/features/incidents/helpers";

	let {
		workspace,
		onOpenResources,
		onOpenArgo,
		onOpenIncidents,
		onOpenPortForwards,
		onOpenLauncher,
	}: {
		workspace: SavedWorkspace;
		onOpenResources: (
			namespace?: string,
			initialSearch?: string,
			initialGitOpsFilter?: string,
			initialHealthFilter?: HealthFilter,
		) => void;
		onOpenArgo: (argoApp?: string) => void;
		onOpenIncidents: (filter?: IncidentFilter) => void;
		onOpenPortForwards: () => void;
		onOpenLauncher: () => void;
	} = $props();

	const client = createTauriClient();

	const sourceQuery = createQuery(() => ({
		queryKey: ["kubeconfig-sources"],
		queryFn: () => getKubeconfigSources(client),
		staleTime: 30_000,
	}));
	const sourceReady = $derived(sourceQuery.isSuccess || sourceQuery.isError);
	const kubeconfigSourceKey = $derived(sourceQuery.data?.sourceKey);
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
	}));
	const namespaces = $derived((namespacesQuery.data ?? []).map((namespace) => namespace.name));

	const kindsQuery = createQuery(() => ({
		queryKey: queryKeys.resourceKinds(workspace.scope.clusterContext, kubeconfigSourceKey),
		queryFn: () => listResourceKinds(client, workspace.scope.clusterContext, kubeconfigSourceKey),
		enabled: sourceReady && clusterAvailable && !contextsQuery.isPending,
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

	const resourcesQuery = createQuery(() => ({
		queryKey: queryKeys.resources(workspaceContextKey, workspaceFetchKeys, kubeconfigSourceKey),
		queryFn: () => fetchWorkspaceResources(workspace.scope, availableNamespaces, kubeconfigSourceKey),
		enabled:
			sourceReady &&
			restoreStatus.clusterAvailable &&
			!namespacesQuery.isPending &&
			!kindsQuery.isPending,
		staleTime: 30_000,
	}));
	const rows = $derived(resourcesQuery.data ?? []);
	const health = $derived(buildWorkspaceHealthSummary(rows));
	const unhealthyCount = $derived(health.attention + health.degraded);
	const hasIncidentShortcuts = $derived(
		unhealthyCount > 0 || health.attention > 0 || health.restarted > 0,
	);

	const argoDetectedQuery = createQuery(() => ({
		queryKey: queryKeys.argoDetect(workspace.scope.clusterContext, kubeconfigSourceKey),
		queryFn: () => detectArgoCD(client, workspace.scope.clusterContext, kubeconfigSourceKey),
		enabled: sourceReady && restoreStatus.clusterAvailable,
	}));
	const fluxDetectedQuery = createQuery(() => ({
		queryKey: queryKeys.fluxDetect(workspace.scope.clusterContext, kubeconfigSourceKey),
		queryFn: () => detectFlux(client, workspace.scope.clusterContext, kubeconfigSourceKey),
		enabled: sourceReady && restoreStatus.clusterAvailable,
		staleTime: 60_000,
	}));
	const argoAppsQuery = createQuery(() => ({
		queryKey: queryKeys.argoApps(workspace.scope.clusterContext, kubeconfigSourceKey),
		queryFn: () => listArgoApplications(client, workspace.scope.clusterContext, kubeconfigSourceKey),
		enabled: argoDetectedQuery.data === true,
	}));
	const fluxResourceQueries = createQueries(() => ({
		queries: (fluxDetectedQuery.data?.kinds ?? []).map((kind) => ({
			queryKey: queryKeys.fluxResources(
				workspace.scope.clusterContext,
				kind,
				kubeconfigSourceKey,
			),
			queryFn: () => listFluxResources(client, workspace.scope.clusterContext, kind, kubeconfigSourceKey),
			enabled: fluxDetectedQuery.data?.detected === true,
			staleTime: 30_000,
		})),
	}));
	const argoDrift = $derived(
		(argoAppsQuery.data ?? []).filter((app) => app.syncStatus && app.syncStatus !== "Synced")
			.length,
	);
	const fluxDetected = $derived(fluxDetectedQuery.data?.detected === true);
	const fluxRows = $derived(
		fluxResourceQueries.flatMap((query) => (query.data as FluxResourceSummary[] | undefined) ?? []),
	);
	const fluxPending = $derived(
		fluxResourceQueries.length > 0 && fluxResourceQueries.some((query) => query.isPending),
	);
	const fluxError = $derived(fluxResourceQueries.some((query) => query.isError));
	const gitOpsDetected = $derived(argoDetectedQuery.data === true || fluxDetected);
	const gitOpsDetecting = $derived(argoDetectedQuery.isPending || fluxDetectedQuery.isPending);
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
</script>

<section class="mx-auto flex w-full max-w-6xl flex-col gap-4 p-4 md:p-6">
	<header class="flex flex-wrap items-center justify-between gap-3 border-b pb-3">
		<div class="min-w-0">
			<h1 class="truncate text-lg font-semibold">{workspace.name}</h1>
			<div class="mt-1 flex flex-wrap gap-2">
				<Badge variant="outline" class="rounded-sm">{workspaceClusterGroupLabel(workspace.scope)}</Badge>
				<Badge variant="outline" class="rounded-sm">
					{workspace.scope.namespaces.length || "All"} namespaces
				</Badge>
				<Badge variant="outline" class="max-w-96 truncate rounded-sm">
					{workspace.scope.kinds.map(resourceKindLabel).join(", ")}
				</Badge>
			</div>
		</div>
		<div class="flex flex-wrap justify-end gap-2">
			<Button type="button" onclick={() => onOpenResources()}>
				<Boxes data-icon="inline-start" /> Resources
			</Button>
			<Button type="button" variant="outline" onclick={onOpenLauncher}>
				<FolderOpen data-icon="inline-start" /> Workspaces
			</Button>
			<Button type="button" variant="outline" onclick={onOpenPortForwards}>
				<Cable data-icon="inline-start" /> Port Forwards
			</Button>
			<Button type="button" variant="outline" onclick={onOpenIncidents}>
				<AlertTriangle data-icon="inline-start" /> Incidents
			</Button>
		</div>
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

	<div class="grid grid-cols-1 gap-2 md:grid-cols-5">
		{@render Metric("Total", health.total)}
		{@render Metric("Healthy", health.healthy, "text-emerald-300")}
		{@render Metric("Needs attention", health.attention, "text-amber-300")}
		{@render Metric("Degraded", health.degraded, "text-red-300")}
		{@render Metric("Restarted", health.restarted, "text-sky-300")}
	</div>

	{#if resourcesQuery.isPending}
		<div class="flex min-h-20 items-center justify-center gap-2 text-sm text-muted-foreground">
			<Spinner class="size-4" /> Loading workspace health...
		</div>
	{:else if resourcesQuery.isError}
		<Alert variant="destructive">
			<AlertTitle>Failed to refresh workspace resources</AlertTitle>
			<AlertDescription>Resource health may be stale until the next refresh succeeds.</AlertDescription>
		</Alert>
	{/if}

	{#if hasIncidentShortcuts}
		<Card>
			<CardHeader>
				<CardTitle>Incident shortcuts</CardTitle>
				<CardDescription>Open Incident Cockpit for this workspace.</CardDescription>
			</CardHeader>
			<CardContent class="flex flex-wrap gap-2">
				{@render IncidentShortcutButton("Unhealthy", unhealthyCount, "unhealthy", onOpenIncidents)}
				{@render IncidentShortcutButton("Warnings", health.attention, "attention", onOpenIncidents)}
				{@render IncidentShortcutButton("Restarted", health.restarted, "restarted", onOpenIncidents)}
			</CardContent>
		</Card>
	{/if}

	<div class="grid gap-4 lg:grid-cols-[minmax(0,1fr)_320px]">
		<Card>
			<CardHeader>
				<CardTitle>Shortcuts</CardTitle>
				<span class="text-xs text-muted-foreground">{workspace.shortcuts.length}</span>
			</CardHeader>
			<CardContent class="flex flex-wrap gap-2">
				{#if workspace.shortcuts.length === 0}
					<Empty class="min-h-32 border-0">
						<EmptyHeader>
							<EmptyTitle>No shortcuts</EmptyTitle>
							<EmptyDescription>Workspace shortcuts appear after scope is saved.</EmptyDescription>
						</EmptyHeader>
					</Empty>
				{/if}
				{#each workspace.shortcuts as shortcut}
					<Button type="button" variant="outline" size="sm" onclick={() => openShortcut(shortcut)}>
						{#if shortcut.kind === "argo"}<GitBranch data-icon="inline-start" />
						{:else if shortcut.kind === "namespace"}<Layers data-icon="inline-start" />
						{:else if shortcut.kind === "compare"}<GitCompareArrows data-icon="inline-start" />
						{:else}<Boxes data-icon="inline-start" />{/if}
						{shortcut.label}
					</Button>
				{/each}
			</CardContent>
		</Card>

		<Card>
			<CardHeader>
				<CardTitle>GitOps</CardTitle>
				<CardDescription>Argo CD and Flux inventory for this cluster.</CardDescription>
			</CardHeader>
			<CardContent>
				{#if gitOpsDetecting}
					<div class="inline-flex items-center gap-2 text-xs text-muted-foreground">
						<Spinner class="size-4" /> Detecting...
					</div>
				{:else if !gitOpsDetected}
					<div class="text-xs text-muted-foreground">Not detected</div>
				{:else}
					<div class="grid gap-2">
						{#if argoDetectedQuery.data === true && argoAppsQuery.isPending}
							<div class="inline-flex items-center gap-2 text-xs text-muted-foreground">
								<Spinner class="size-4" /> Loading applications...
							</div>
						{:else if argoAppsQuery.isError || fluxError}
							<Alert variant="destructive">
								<AlertTitle>Failed to load GitOps inventory</AlertTitle>
								<AlertDescription>Provider counts are unavailable until refresh.</AlertDescription>
							</Alert>
						{:else if argoDetectedQuery.data === true}
							<div class="flex items-center justify-between py-2 text-xs">
								<span class="text-muted-foreground">Applications</span>
								<strong>{argoAppsQuery.data?.length ?? 0}</strong>
							</div>
							<Separator />
							<div class="flex items-center justify-between py-2 text-xs">
								<span class="text-muted-foreground">Out of sync</span>
								<strong class="text-amber-300">{argoDrift}</strong>
							</div>
							<Separator />
						{/if}
						{#if fluxDetected}
							{#if fluxPending}
								<div class="inline-flex items-center gap-2 text-xs text-muted-foreground">
									<Spinner class="size-4" /> Loading Flux resources...
								</div>
							{:else}
								<div class="flex items-center justify-between py-2 text-xs">
									<span class="text-muted-foreground">Flux resources</span>
									<strong>{fluxRows.length}</strong>
								</div>
								<Separator />
							{/if}
						{/if}
						<Button type="button" variant="outline" onclick={() => onOpenArgo()}>
							<GitBranch data-icon="inline-start" /> Open GitOps
						</Button>
					</div>
				{/if}
			</CardContent>
		</Card>
	</div>

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

{#snippet Metric(label: string, value: string | number, tone = "")}
	<Card size="sm" class="min-h-20 justify-center">
		<CardContent class="flex flex-col gap-1">
			<span class="text-[0.72rem] font-semibold uppercase text-muted-foreground">{label}</span>
			<strong class={tone}>{value}</strong>
		</CardContent>
	</Card>
{/snippet}

{#snippet IncidentShortcutButton(label: string, count: number, filter: IncidentFilter, onOpenIncidents: (filter?: IncidentFilter) => void)}
	{#if count > 0}
		<Button type="button" variant="outline" size="sm" onclick={() => onOpenIncidents(filter)}>
			<AlertTriangle data-icon="inline-start" />
			{label}
			<Badge variant="secondary" class="ml-1 rounded-sm px-1.5">{count}</Badge>
		</Button>
	{/if}
{/snippet}

{#snippet CompareSide(
	label: string,
	health: { total: number; healthy: number; attention: number; degraded: number; restarted: number },
)}
	<div class="rounded border bg-background p-2">
		<div class="truncate font-medium">{label}</div>
		<div class="mt-1 text-muted-foreground">
			{health.total} total / {health.healthy} healthy / {health.degraded + health.attention} flagged
		</div>
	</div>
{/snippet}
