<script lang="ts">
	import { createQuery } from "@tanstack/svelte-query";
	import FriendlyError from "@/components/FriendlyError.svelte";
	import { ScrollArea } from "@/components/ui/svelte";
	import {
		createTauriClient,
		detectArgoCD,
		detectFlux,
		getKubeconfigSources,
		listNamespaces,
		listResourceKinds,
	} from "@/lib/tauri";
	import { queryKeys } from "@/lib/queryKeys";
	import type {
		DiscoveredResourceKind,
		FluxDetectionSummary,
		KubeconfigSourcesSummary,
		NamespaceSummary,
	} from "@/lib/types";
	import { nodeIdToString, type TreeNode, type TreeNodeId } from "@/lib/tree-nav";
	import { settingsStore } from "@/lib/settings-store";
	import { buildNamespaceTreeNode } from "@/components/sidebar-tree-helpers";
	import { buildSidebarTree, extraDiscoveredKinds } from "./workspaceShellModel";
	import SidebarTreeNode from "./SidebarTreeNode.svelte";

	let {
		clusterContext,
		selectedNode,
		expandedSections,
		onNodeSelect,
		onSectionToggle,
	}: {
		clusterContext: string;
		selectedNode: TreeNodeId | null;
		expandedSections: string[];
		onNodeSelect: (id: TreeNodeId) => void;
		onSectionToggle: (id: string) => void;
	} = $props();

	const client = createTauriClient();
	const showUnavailableGitOpsProviders = $derived(
		$settingsStore.showUnavailableGitOpsProviders,
	);

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

	const resourceKindsQuery = createQuery<DiscoveredResourceKind[]>(() => ({
		queryKey: queryKeys.resourceKinds(clusterContext, kubeconfigSourceKey),
		queryFn: () => listResourceKinds(client, clusterContext, kubeconfigSourceKey),
		enabled: Boolean(clusterContext) && sourceReady,
	}));

	const argoDetectionQuery = createQuery<boolean>(() => ({
		queryKey: queryKeys.argoDetect(clusterContext, kubeconfigSourceKey),
		queryFn: () => detectArgoCD(client, clusterContext, kubeconfigSourceKey),
		enabled: Boolean(clusterContext) && sourceReady,
		staleTime: 60_000,
	}));

	const fluxDetectionQuery = createQuery<FluxDetectionSummary>(() => ({
		queryKey: queryKeys.fluxDetect(clusterContext, kubeconfigSourceKey),
		queryFn: () => detectFlux(client, clusterContext, kubeconfigSourceKey),
		enabled: Boolean(clusterContext) && sourceReady,
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
	const extraKinds = $derived(extraDiscoveredKinds(resourceKindsQuery.data ?? []));
	function getLazyChildren(node: TreeNode) {
		if (node.id.type !== "namespace" || !node.id.namespace) return node.children;
		return buildNamespaceTreeNode(node.id.namespace, extraKinds).children;
	}
	const nodes = $derived(buildSidebarTree({
		namespaces: namespacesQuery.data ?? [],
		resourceKinds: resourceKindsQuery.data ?? [],
		argoDetected: argoDetectionQuery.data,
		fluxDetection: fluxDetectionQuery.data,
		detectingGitOps: argoDetectionQuery.isPending || fluxDetectionQuery.isPending,
		resourceKindsPending: resourceKindsQuery.isPending,
		resourceKindsError,
		showUnavailableGitOpsProviders,
	}));
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
	<ScrollArea class="min-h-0 flex-1">
		<nav class="py-2" aria-label="Kubernetes resource tree">
			<ul class="m-0 list-none p-0" role="tree">
				{#each nodes as node (nodeIdToString(node.id))}
					<SidebarTreeNode
						{node}
						depth={0}
						{selectedNode}
						{expandedSections}
						{onNodeSelect}
						{onSectionToggle}
						{getLazyChildren}
					/>
				{/each}
			</ul>
		</nav>
	</ScrollArea>
{/if}
