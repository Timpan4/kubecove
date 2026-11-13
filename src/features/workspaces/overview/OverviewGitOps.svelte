<script lang="ts">
	import { createQueries, createQuery } from "@tanstack/svelte-query";
	import { GitBranch } from "lucide-svelte";
	import FriendlyError from "@/components/FriendlyError.svelte";
	import { Badge, Button, Card, CardContent, CardDescription, CardHeader, CardTitle, Separator, Spinner } from "@/components/ui/svelte";
	import { healthStateLabel } from "@/lib/resource-health";
	import { gitOpsSelectionResource } from "@/lib/gitops-resource";
	import type { ResourceSummary } from "@/lib/types";
	import { loadingDelay } from "@/lib/loading-delay.svelte";
	import { queryKeys } from "@/lib/queryKeys";
	import { createTauriClient, detectArgoCD, detectFlux, listArgoApplications, listFluxResources } from "@/lib/tauri";
	let { clusterContext, kubeconfigSourceKey, enabled, onOpenArgo, onOpenResource }: {
		clusterContext: string; kubeconfigSourceKey: string | undefined; enabled: boolean;
		onOpenArgo: (name?: string, namespace?: string) => void;
		onOpenResource: (resource: ResourceSummary) => void;
	} = $props();
	const client = createTauriClient();
	const argoDetectedQuery = createQuery(() => ({
		queryKey: queryKeys.argoDetect(clusterContext, kubeconfigSourceKey),
		queryFn: () => detectArgoCD(client, clusterContext, kubeconfigSourceKey),
		enabled: enabled,
	}));
	const fluxDetectedQuery = createQuery(() => ({
		queryKey: queryKeys.fluxDetect(clusterContext, kubeconfigSourceKey),
		queryFn: () => detectFlux(client, clusterContext, kubeconfigSourceKey),
		enabled: enabled,
		staleTime: 60_000,
	}));
	const argoAppsQuery = createQuery(() => ({
		queryKey: queryKeys.argoApps(clusterContext, kubeconfigSourceKey),
		queryFn: () => listArgoApplications(client, clusterContext, kubeconfigSourceKey),
		enabled: argoDetectedQuery.data === true,
	}));
	const fluxResourceQueries = createQueries(() => ({
		queries: (fluxDetectedQuery.data?.kinds ?? []).map((kind) => ({
			queryKey: queryKeys.fluxResources(
				clusterContext,
				kind,
				kubeconfigSourceKey,
			),
			queryFn: () => listFluxResources(client, clusterContext, kind, kubeconfigSourceKey),
			enabled: fluxDetectedQuery.data?.detected === true,
			staleTime: 30_000,
		})),
	}));
	const argoDrift = $derived(
		(argoAppsQuery.data?.items ?? []).filter((app) => app.syncStatus && app.syncStatus !== "Synced")
			.length,
	);
	const fluxDetected = $derived(fluxDetectedQuery.data?.detected === true);
	const fluxRows = $derived(
		fluxResourceQueries.flatMap((query) => query.data?.items ?? []),
	);
	const fluxPending = $derived(fluxResourceQueries.some((query) => query.isPending));
	const fluxError = $derived(fluxResourceQueries.find((query) => query.isError)?.error ?? null);
	const gitOpsInventoryError = $derived(argoAppsQuery.error ?? fluxError);
	const gitOpsDetected = $derived(argoDetectedQuery.data === true || fluxDetected);
	const gitOpsDetecting = loadingDelay(() => argoDetectedQuery.isPending || fluxDetectedQuery.isPending);
	const applicationsLoading = loadingDelay(() => argoAppsQuery.isPending);
	const fluxLoading = loadingDelay(() => fluxPending);

	const detectionError = $derived(argoDetectedQuery.error ?? fluxDetectedQuery.error);
	const troubled = $derived((argoAppsQuery.data?.items ?? []).filter(app => app.healthAssessment?.state === "degraded" || app.healthAssessment?.state === "needsAttention"));
</script>
<Card size="sm" elevation="flat">
	<CardHeader class="border-b"><CardTitle>GitOps</CardTitle><CardDescription>Argo CD and Flux inventory.</CardDescription></CardHeader>
	<CardContent class="grid gap-2">
	{#if gitOpsDetecting.active}
		{#if gitOpsDetecting.shown}
			<div class="inline-flex items-center gap-2 text-xs text-muted-foreground">
				<Spinner class="size-4" /> Detecting...
			</div>
		{/if}
	{:else if !gitOpsDetected && !detectionError}
		<div class="text-xs text-muted-foreground">Not detected</div>
	{:else}
		{#if detectionError}
			<FriendlyError mode="compact" error={detectionError} context={{ operation: "providerDetection", fallbackTitle: "Failed to detect GitOps providers", partial: true }} />
		{/if}
		<div class="grid gap-2">
			{#if !argoAppsQuery.isPending && !argoAppsQuery.isError}
				{#each troubled as app (`${app.namespace}/${app.name}`)}
					<Button variant="ghost" aria-label={`Inspect Application ${app.name} in ${app.namespace}`} class="h-auto justify-start whitespace-normal text-left" onclick={() => onOpenResource(gitOpsSelectionResource({ type: "argoApp", item: app }))}>
						<span class="min-w-0 flex-1 font-mono break-all">{app.name}</span>
						<Badge variant="outline" class={app.healthAssessment?.state === "degraded" ? "text-red-300" : "text-amber-300"}>{healthStateLabel(app.healthAssessment?.state ?? "unknown")}</Badge>
					</Button>
				{/each}
			{/if}
			{#if argoDetectedQuery.data === true && applicationsLoading.active}
				{#if applicationsLoading.shown}
					<div class="inline-flex items-center gap-2 text-xs text-muted-foreground">
						<Spinner class="size-4" /> Loading applications...
					</div>
				{/if}
			{:else if gitOpsInventoryError}
				<FriendlyError
					mode="compact"
					error={gitOpsInventoryError}
					context={{
						operation: "providerDetection",
						fallbackTitle: "Failed to load GitOps inventory",
						partial: true,
					}}
				/>
			{:else if argoDetectedQuery.data === true}
				<div class="flex items-center justify-between py-2 text-xs">
					<span class="text-muted-foreground">Applications</span>
					<strong>{argoAppsQuery.data?.items.length ?? 0}</strong>
				</div>
				<Separator />
				<div class="flex items-center justify-between py-2 text-xs">
					<span class="text-muted-foreground">Out of sync</span>
					<strong class={argoDrift > 0 ? "text-amber-300" : ""}>{argoDrift}</strong>
				</div>
				<Separator />
			{/if}
			{#if fluxDetected}
				{#if fluxLoading.active}
					{#if fluxLoading.shown}
						<div class="inline-flex items-center gap-2 text-xs text-muted-foreground">
							<Spinner class="size-4" /> Loading Flux resources...
						</div>
					{/if}
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
