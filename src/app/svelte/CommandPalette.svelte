<script lang="ts">
	import { createQuery, useQueryClient } from "@tanstack/svelte-query";
	import { onMount } from "svelte";
	import { Box, Folder, PanelsTopLeft } from "lucide-svelte";
	import {
		CommandDialog,
		CommandEmpty,
		CommandGroup,
		CommandInput,
		CommandItem,
		CommandList,
	} from "@/components/ui/svelte";
	import {
		buildNavigationEntries,
		dedupeResources,
		filterNamespaces,
		filterNavigationEntries,
		resourceEntryKey,
		type PaletteNavigationEntry,
	} from "@/features/command-palette/entries";
	import { shouldToggleCommandPaletteShortcut } from "@/features/command-palette/shortcut";
	import {
		buildFetchKeys,
		buildResourceSearchIndex,
		fetchResourcePage,
		filterResourceSearchIndex,
	} from "@/features/resources";
	import {
		createTauriClient,
		detectArgoCD,
		detectFlux,
		getKubeconfigSources,
		listNamespaces,
	} from "@/lib/tauri";
	import { queryKeys } from "@/lib/queryKeys";
	import { kubeconfigSourceKey as normalizeKubeconfigSourceKey } from "@/lib/settings";
	import {
		makeNamespaceNode,
		type TreeNodeId,
	} from "@/lib/tree-nav";
	import type { ResourceSummary } from "@/lib/types";
	import type { SavedWorkspace } from "@/lib/workspace-model";
	import { settingsStore } from "@/lib/settings-store";
	import { treeNodeForResource } from "./workspaceShellModel";

	const RESOURCE_RESULT_CAP = 50;
	const NAMESPACE_RESULT_CAP = 8;

	let {
		open = $bindable(false),
	workspace,
	onNodeSelect,
	onResourceSelect,
	onOpenLauncher,
	onOpenSettings,
}: {
	open: boolean;
	workspace: SavedWorkspace;
	onNodeSelect: (id: TreeNodeId) => void;
	onResourceSelect: (resource: ResourceSummary, id: TreeNodeId) => void;
	onOpenLauncher: () => void;
	onOpenSettings: () => void;
} = $props();

	const client = createTauriClient();
	const queryClient = useQueryClient();
	let query = $state("");
	const showUnavailableGitOpsProviders = $derived(
		$settingsStore.showUnavailableGitOpsProviders,
	);

	const sourceQuery = createQuery(() => ({
		queryKey: ["kubeconfig-sources"],
		queryFn: () => getKubeconfigSources(client),
		staleTime: 30_000,
	}));
	const sourceReady = $derived(sourceQuery.isSuccess || sourceQuery.isError);
	const kubeconfigSourceKey = $derived(sourceQuery.data?.sourceKey);
	const normalizedSourceKey = $derived(normalizeKubeconfigSourceKey(kubeconfigSourceKey));
	const fetchKeys = $derived(buildFetchKeys(workspace.scope.namespaces, workspace.scope.kinds));

	const namespacesQuery = createQuery<string[]>(() => ({
		queryKey: queryKeys.namespaces(workspace.scope.clusterContext, kubeconfigSourceKey),
		queryFn: async () =>
			(await listNamespaces(client, workspace.scope.clusterContext, kubeconfigSourceKey)).map(
				(namespace) => namespace.name,
			),
		enabled: open && sourceReady,
		staleTime: 30_000,
	}));

	const resourcesQuery = createQuery<ResourceSummary[]>(() => ({
		queryKey: queryKeys.resources(workspace.scope.clusterContext, fetchKeys, kubeconfigSourceKey),
		queryFn: () => fetchResourcePage(workspace.scope.clusterContext, fetchKeys, kubeconfigSourceKey),
		enabled: open && sourceReady && fetchKeys.length > 0,
		staleTime: 30_000,
	}));

	const argoDetectionQuery = createQuery<boolean>(() => ({
		queryKey: queryKeys.argoDetect(workspace.scope.clusterContext, kubeconfigSourceKey),
		queryFn: () => detectArgoCD(client, workspace.scope.clusterContext, kubeconfigSourceKey),
		enabled: open && sourceReady,
		staleTime: 60_000,
	}));

	const fluxDetectionQuery = createQuery(() => ({
		queryKey: queryKeys.fluxDetect(workspace.scope.clusterContext, kubeconfigSourceKey),
		queryFn: () => detectFlux(client, workspace.scope.clusterContext, kubeconfigSourceKey),
		enabled: open && sourceReady,
		staleTime: 60_000,
	}));

	const gitOpsNavigationVisible = $derived(
		argoDetectionQuery.data === true ||
			fluxDetectionQuery.data?.detected === true ||
			showUnavailableGitOpsProviders,
	);
	const navigationEntries = $derived(buildNavigationEntries(gitOpsNavigationVisible));
	const hasQuery = $derived(query.trim().length > 0);
	const visibleNavigation = $derived(filterNavigationEntries(navigationEntries, query));
	const visibleNamespaces = $derived(
		hasQuery
			? filterNamespaces(namespacesQuery.data ?? [], query).slice(0, NAMESPACE_RESULT_CAP)
			: [],
	);
	const resourceSearchIndex = $derived.by(() => {
		if (!open) return [];
		const warmedRows = resourcesQuery.data ?? [];
		const cached = queryClient.getQueriesData<ResourceSummary[]>({
			queryKey: ["resources", normalizedSourceKey, workspace.scope.clusterContext],
		});
		const merged = [
			...warmedRows,
			...cached.flatMap(([, rows]) => rows ?? []),
		];
		return buildResourceSearchIndex(dedupeResources(merged));
	});
	const visibleResources = $derived(
		hasQuery
			? filterResourceSearchIndex(resourceSearchIndex, query, "").slice(0, RESOURCE_RESULT_CAP)
			: [],
	);
	const hasResults = $derived(
		visibleNavigation.length > 0 || visibleNamespaces.length > 0 || visibleResources.length > 0,
	);

	onMount(() => {
		const handler = (event: KeyboardEvent) => {
			if (!shouldToggleCommandPaletteShortcut(event)) return;
			event.preventDefault();
			event.stopPropagation();
			open = !open;
		};
		window.addEventListener("keydown", handler, { capture: true });
		return () => {
			window.removeEventListener("keydown", handler, { capture: true });
		};
	});

	function close() {
		open = false;
		query = "";
	}

	function selectNavigation(entry: PaletteNavigationEntry) {
		close();
		if (entry.action === "settings") onOpenSettings();
		else if (entry.action === "launcher") onOpenLauncher();
		else if (entry.nodeId) onNodeSelect(entry.nodeId);
	}

	function selectNamespace(namespace: string) {
		close();
		onNodeSelect(makeNamespaceNode(namespace).id);
	}

	function selectResource(resource: ResourceSummary) {
		close();
		onResourceSelect(resource, treeNodeForResource(resource));
	}

</script>

<CommandDialog
	{open}
	onOpenChange={(next: boolean) => {
		open = next;
		if (!next) query = "";
	}}
	title="Search"
	description="Search views, namespaces, and resources"
	commandProps={{ shouldFilter: false }}
>
	<CommandInput
		value={query}
		oninput={(event: Event) => {
			query = event.currentTarget instanceof HTMLInputElement ? event.currentTarget.value : "";
		}}
		placeholder="Search views, namespaces, and resources..."
	/>
	<CommandList>
		{#if !hasResults}
			<CommandEmpty>No results found.</CommandEmpty>
		{/if}
		{#if visibleResources.length > 0}
			<CommandGroup heading="Resources">
				{#each visibleResources as resource}
					<CommandItem value={resourceEntryKey(resource)} onSelect={() => selectResource(resource)}>
						<Box class="shrink-0 text-muted-foreground" />
						<span class="truncate">{resource.name}</span>
						<span class="ml-auto flex shrink-0 items-center gap-2 text-[10px] text-muted-foreground">
							{#if resource.namespace}<span>{resource.namespace}</span>{/if}
							<span>{resource.kind}</span>
						</span>
					</CommandItem>
				{/each}
			</CommandGroup>
		{/if}
		{#if visibleNamespaces.length > 0}
			<CommandGroup heading="Namespaces">
				{#each visibleNamespaces as namespace}
					<CommandItem value={`namespace:${namespace}`} onSelect={() => selectNamespace(namespace)}>
						<Folder class="shrink-0 text-muted-foreground" />
						<span class="truncate">{namespace}</span>
					</CommandItem>
				{/each}
			</CommandGroup>
		{/if}
		{#if visibleNavigation.length > 0}
			<CommandGroup heading="Go to">
				{#each visibleNavigation as entry}
					<CommandItem value={entry.id} onSelect={() => selectNavigation(entry)}>
						<PanelsTopLeft class="shrink-0 text-muted-foreground" />
						<span class="truncate">{entry.label}</span>
					</CommandItem>
				{/each}
			</CommandGroup>
		{/if}
	</CommandList>
</CommandDialog>
