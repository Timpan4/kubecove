<script lang="ts">
	import { tick, untrack } from "svelte";
	import { createQueries, createQuery } from "@tanstack/svelte-query";
	import FriendlyError from "@/components/FriendlyError.svelte";
	import {
		Badge,
		Button,
		ScrollArea,
		Tooltip,
		TooltipContent,
		TooltipTrigger,
	} from "@/components/ui/svelte";
	import { cnfast } from "@/lib/utils";
	import {
		createTauriClient,
		detectArgoCD,
		detectFlux,
		listNamespaces,
		listPresentCustomResourceKinds,
		listResourceKinds,
	} from "@/lib/tauri";
	import { finiteReadMeta } from "@/lib/finite-read-lifecycle";
	import { queryKeys } from "@/lib/queryKeys";
	import type {
		DiscoveredResourceKind,
		FluxDetectionSummary,
		NamespaceSummary,
	} from "@/lib/types";
	import type { WorkspaceReadContext } from "@/lib/workspaceReadContext";
	import { nodeIdToString, type TreeNode, type TreeNodeId } from "@/lib/tree-nav";
	import { settingsStore } from "@/lib/settings-store";
	import {
		buildExpandedNamespaceChildren,
		customResourceErrorsByExpandedNamespace,
		customResourcesByExpandedNamespace,
		pendingCustomResourceNamespaces as buildPendingCustomResourceNamespaces,
	} from "@/components/sidebar-tree-helpers";
	import { buildSidebarTree, collectSelectedAncestorIds, sidebarSelectionNode } from "./workspaceShellModel";
	import SidebarTreeNode from "./SidebarTreeNode.svelte";

	let {
		workspaceReadContext,
		selectedNode,
		sidebarNavigationRevision,
		expandedSections,
		onNodeSelect,
		onSectionToggle,
		scrollTop = $bindable(0),
		previousSelectedKey = $bindable(),
		revealKey = $bindable(null),
	}: {
		workspaceReadContext: WorkspaceReadContext;
		selectedNode: TreeNodeId | null;
		sidebarNavigationRevision: number;
		expandedSections: string[];
		onNodeSelect: (id: TreeNodeId) => void;
		onSectionToggle: (id: string) => void;
		scrollTop?: number;
		previousSelectedKey?: string | null;
		revealKey?: string | null;
	} = $props();

	const selectionKey = $derived(selectedNode ? `${nodeIdToString(selectedNode)}:${sidebarNavigationRevision}` : null);
	if (previousSelectedKey === undefined) previousSelectedKey = untrack(() => selectionKey);
	$effect(() => {
		const key = selectionKey;
		if (key === previousSelectedKey) return;
		previousSelectedKey = key;
		void tick().then(() => {
			if (selectionKey === key) revealKey = key;
		});
	});

	function selectFromTree(node: TreeNodeId) {
		previousSelectedKey = `${nodeIdToString(node)}:${sidebarNavigationRevision}`;
		revealKey = null;
		onNodeSelect(node);
	}

	const client = createTauriClient();
	const showUnavailableGitOpsProviders = $derived(
		$settingsStore.showUnavailableGitOpsProviders,
	);
	const showCustomResources = $derived($settingsStore.showCustomResources);
	const clusterContext = $derived(workspaceReadContext.clusterContext);
	const sourceReady = $derived(workspaceReadContext.sourceReady);
	const kubeconfigSourceKey = $derived(workspaceReadContext.kubeconfigSourceKey);

	const namespacesQuery = createQuery<NamespaceSummary[]>(() => ({
		queryKey: queryKeys.namespaces(clusterContext, kubeconfigSourceKey),
		queryFn: () => listNamespaces(client, clusterContext, kubeconfigSourceKey),
		enabled: Boolean(clusterContext) && sourceReady,
		retry: false,
	}));

	const resourceKindsQuery = createQuery<DiscoveredResourceKind[]>(() => ({
		queryKey: queryKeys.resourceKinds(clusterContext, kubeconfigSourceKey),
		queryFn: () => listResourceKinds(client, clusterContext, kubeconfigSourceKey),
		enabled: showCustomResources && Boolean(clusterContext) && sourceReady,
		retry: false,
	}));

	const argoDetectionQuery = createQuery<boolean>(() => ({
		queryKey: queryKeys.argoDetect(clusterContext, kubeconfigSourceKey),
		queryFn: () => detectArgoCD(client, clusterContext, kubeconfigSourceKey),
		enabled: Boolean(clusterContext) && sourceReady && !namespacesQuery.isPending,
		staleTime: 60_000,
	}));

	const fluxDetectionQuery = createQuery<FluxDetectionSummary>(() => ({
		queryKey: queryKeys.fluxDetect(clusterContext, kubeconfigSourceKey),
		queryFn: () => detectFlux(client, clusterContext, kubeconfigSourceKey),
		enabled: Boolean(clusterContext) && sourceReady && !namespacesQuery.isPending,
		staleTime: 60_000,
	}));

	const namespaceError = $derived(
		namespacesQuery.isError ? namespacesQuery.error : null,
	);
	const resourceKindsError = $derived(
		resourceKindsQuery.error instanceof Error
			? resourceKindsQuery.error.message
			: resourceKindsQuery.error
				? String(resourceKindsQuery.error)
				: "",
	);
	const expandedNamespaces = $derived(
		(namespacesQuery.data ?? [])
			.map((namespace) => namespace.name)
			.filter((namespace) =>
				expandedSections.includes(
					nodeIdToString({ type: "namespace", section: "namespaces", namespace }),
				),
			),
	);
	const namespaceCustomResourceQueries = createQueries(() => ({
		queries: expandedNamespaces.map((namespace) => ({
			queryKey: queryKeys.presentCustomResourceKinds(
				clusterContext,
				[namespace],
				kubeconfigSourceKey,
			),
			queryFn: () =>
				listPresentCustomResourceKinds(client, clusterContext, [namespace], kubeconfigSourceKey),
			enabled: showCustomResources && Boolean(clusterContext) && sourceReady,
			staleTime: 30_000,
			retry: false,
			meta: finiteReadMeta({ namespace }),
		})),
	}));
	const customResourcesByNamespace = $derived.by(() => {
		return showCustomResources
			? customResourcesByExpandedNamespace(expandedNamespaces, namespaceCustomResourceQueries)
			: new Map<string, DiscoveredResourceKind[]>();
	});
	const customResourceErrorsByNamespace = $derived.by(() => {
		return showCustomResources
			? customResourceErrorsByExpandedNamespace(expandedNamespaces, namespaceCustomResourceQueries)
			: new Map<string, string>();
	});
	const pendingCustomResourceNamespaces = $derived.by(() => {
		return showCustomResources
			? buildPendingCustomResourceNamespaces(expandedNamespaces, namespaceCustomResourceQueries)
			: new Set<string>();
	});
	let customResourceSearch = $state("");
	function getLazyChildren(node: TreeNode) {
		if (node.id.type !== "namespace" || !node.id.namespace) return node.children;
		if (!showCustomResources) return node.children;
		const customResources = customResourcesByNamespace.get(node.id.namespace) ?? [];
		return buildExpandedNamespaceChildren(
			node.id.namespace,
			customResources,
			customResourceErrorsByNamespace.get(node.id.namespace),
			pendingCustomResourceNamespaces.has(node.id.namespace),
		);
	}
	const groups = $derived(buildSidebarTree({
		namespaces: namespacesQuery.data ?? [],
		resourceKinds: resourceKindsQuery.data ?? [],
		argoDetected: argoDetectionQuery.data,
		fluxDetection: fluxDetectionQuery.data,
		detectingGitOps: argoDetectionQuery.isPending || fluxDetectionQuery.isPending,
		resourceKindsPending: resourceKindsQuery.isPending,
		resourceKindsError,
		showUnavailableGitOpsProviders,
		showCustomResources,
		customResourceSearch,
	}));
	const sidebarSelectedNode = $derived(sidebarSelectionNode(selectedNode, groups));
	const selectedAncestorIds = $derived(
		collectSelectedAncestorIds(groups, sidebarSelectedNode, getLazyChildren),
	);
	const isNamespaceListSelected = $derived(
		selectedNode?.type === "section" && selectedNode.section === "namespaces",
	);
	const namespaceScope = $derived(workspaceReadContext.namespaceScope);
	const groupHeadings = { workspace: "Workspace", cluster: "Cluster", namespaces: "Namespaces" };
	const uid = $props.id();
</script>

{#if !clusterContext}
	<div class="p-4 text-center text-xs text-muted-foreground">
		Select a workspace cluster context to load the resource tree
	</div>
{:else}
	{#if namespaceError}
		<FriendlyError
			mode="compact"
			class="m-2"
			error={namespaceError}
			context={{
				operation: "resourcesLoad",
				fallbackTitle: "Namespaces failed to load",
				partial: true,
			}}
		/>
	{/if}
	<ScrollArea class="min-h-0 flex-1" bind:scrollTop>
		<nav class="py-2" aria-label="Kubernetes resource tree">
			{#each groups as group (group.id)}
				{@const headingId = `${uid}-${group.id}`}
				<div class="flex items-center justify-between gap-2 px-3 pt-3 pb-1">
					<div class="flex min-w-0 items-center gap-1.5">
						<h2
							id={headingId}
							class="min-w-0 truncate text-[10.5px] font-semibold uppercase tracking-wide text-muted-foreground"
						>
							{#if group.id === "scope"}
								{namespaceScope.length === 0 ? "All namespaces" : "Workspace namespaces"}
							{:else}
								{groupHeadings[group.id]}
							{/if}
						</h2>
						{#if group.id === "scope" && namespaceScope.length > 0}
							<Tooltip>
								<TooltipTrigger
									type="button"
									class="inline-flex rounded-full focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
									aria-label={`Scoped namespaces: ${namespaceScope.join(", ")}`}
								>
									<Badge variant="outline" class="h-4 px-1.5 text-[10px] tabular-nums">
										{namespaceScope.length}
									</Badge>
								</TooltipTrigger>
								<TooltipContent>{namespaceScope.join(", ")}</TooltipContent>
							</Tooltip>
						{/if}
					</div>
					{#if group.id === "namespaces"}
						<Button
							type="button"
							variant="ghost"
							size="xs"
							class={cnfast(
								"text-[10.5px] font-medium",
								isNamespaceListSelected ? "text-primary" : "text-muted-foreground",
							)}
							onclick={() => selectFromTree({ type: "section", section: "namespaces" })}
						>
							View all
						</Button>
					{/if}
				</div>
				<ul class="m-0 list-none p-0" role="tree" aria-labelledby={headingId}>
					{#each group.nodes as node (nodeIdToString(node.id))}
						<SidebarTreeNode
							{node}
							depth={0}
							selectedNode={sidebarSelectedNode}
							revealKey={revealKey === selectionKey ? revealKey : null}
							onReveal={() => (revealKey = null)}
							{selectedAncestorIds}
							{expandedSections}
							onNodeSelect={selectFromTree}
							{onSectionToggle}
							{getLazyChildren}
							{customResourceSearch}
							onCustomResourceSearchChange={(value) => (customResourceSearch = value)}
						/>
					{/each}
				</ul>
			{/each}
		</nav>
	</ScrollArea>
{/if}
