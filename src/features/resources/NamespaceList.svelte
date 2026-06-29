<script lang="ts">
	import { createQuery } from "@tanstack/svelte-query";
	import FriendlyError from "@/components/FriendlyError.svelte";
	import {
		Badge,
		Empty,
		EmptyDescription,
		EmptyHeader,
		EmptyTitle,
		Spinner,
		Table,
		TableBody,
		TableCell,
		TableHead,
		TableHeader,
		TableRow,
	} from "@/components/ui/svelte";
	import { queryKeys } from "@/lib/queryKeys";
	import {
		createTauriClient,
		getKubeconfigSources,
		listNamespaces,
	} from "@/lib/tauri";
	import type { KubeconfigSourcesSummary, NamespaceSummary } from "@/lib/types";

	let { clusterContext }: { clusterContext: string } = $props();

	const client = createTauriClient();
	const sourceQuery = createQuery<KubeconfigSourcesSummary>(() => ({
		queryKey: ["kubeconfig-sources"] as const,
		queryFn: () => getKubeconfigSources(client),
		staleTime: 60_000,
	}));
	const sourceReady = $derived(sourceQuery.isSuccess || sourceQuery.isError);
	const kubeconfigSourceKey = $derived(sourceQuery.data?.sourceKey);
	const namespacesQuery = createQuery<NamespaceSummary[]>(() => ({
		queryKey: queryKeys.namespaces(clusterContext, kubeconfigSourceKey),
		queryFn: () => listNamespaces(client, clusterContext, kubeconfigSourceKey),
		enabled: Boolean(clusterContext) && sourceReady,
	}));

	const namespaces = $derived(namespacesQuery.data ?? []);
	const namespaceError = $derived(
		sourceQuery.isError
			? sourceQuery.error
			: namespacesQuery.isError
				? namespacesQuery.error
				: null,
	);
	const loading = $derived(Boolean(clusterContext) && (!sourceReady || namespacesQuery.isPending));
</script>

{#if !clusterContext}
	<Empty class="min-h-40 border border-dashed">
		<EmptyHeader>
			<EmptyTitle>No cluster context</EmptyTitle>
			<EmptyDescription>Select a workspace with a cluster context.</EmptyDescription>
		</EmptyHeader>
	</Empty>
{:else if namespaceError}
	<FriendlyError
		error={namespaceError}
		context={{ operation: "resourcesLoad", fallbackTitle: "Failed to load namespaces" }}
	/>
{:else if loading}
	<div class="flex min-h-40 items-center justify-center gap-2 text-xs text-muted-foreground">
		<Spinner />
		<span>Loading namespaces</span>
	</div>
{:else if namespaces.length === 0}
	<Empty class="min-h-40 border border-dashed">
		<EmptyHeader>
			<EmptyTitle>No namespaces found</EmptyTitle>
			<EmptyDescription>No namespaces returned for this context.</EmptyDescription>
		</EmptyHeader>
	</Empty>
{:else}
	<div class="flex flex-col gap-3">
		<div class="flex items-center justify-between gap-3">
			<div class="text-sm font-medium">Namespaces</div>
			<Badge variant="outline" class="tabular-nums">{namespaces.length}</Badge>
		</div>
		<Table aria-label="Namespaces">
			<TableHeader>
				<TableRow>
					<TableHead>Name</TableHead>
					<TableHead>Age</TableHead>
					<TableHead>Created</TableHead>
				</TableRow>
			</TableHeader>
			<TableBody>
				{#each namespaces as namespace (namespace.name)}
					<TableRow>
						<TableCell class="font-medium">{namespace.name}</TableCell>
						<TableCell>{namespace.age}</TableCell>
						<TableCell>{namespace.createdAt ?? "-"}</TableCell>
					</TableRow>
				{/each}
			</TableBody>
		</Table>
	</div>
{/if}
