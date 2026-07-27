<script lang="ts">
	import { createQueries, createQuery } from "@tanstack/svelte-query";
	import FriendlyError from "@/components/FriendlyError.svelte";
	import { ScrollArea } from "@/components/ui/svelte";
	import {
		createTauriClient,
		detectArgoCD,
		detectFlux,
		listNamespaces,
		listPresentCustomResourceKinds,
		listResourceKinds,
	} from "@/lib/tauri";
	import { queryKeys } from "@/lib/queryKeys";
	import type {
		DiscoveredResourceKind,
		FluxDetectionSummary,
		NamespaceSummary,
	} from "@/lib/types";
	import type { WorkspaceReadContext } from "@/lib/workspaceReadContext";
	import { nodeIdToString, type TreeNode, type TreeNodeId } from "@/lib/tree-nav";
	import { settingsStore } from "@/lib/settings-store";
	import { buildNamespaceTreeNode } from "@/components/sidebar-tree-helpers";
	import { buildSidebarTree } from "./workspaceShellModel";
	import SidebarTreeNode from "./SidebarTreeNode.svelte";

	let {
		workspaceReadContext,
		selectedNode,
		expandedSections,
		onNodeSelect,
		onSectionToggle,
	}: {
		workspaceReadContext: WorkspaceReadContext;
		selectedNode: TreeNodeId | null;
		expandedSections: string[];
		onNodeSelect: (id: TreeNodeId) => void;
		onSectionToggle: (id: string) => void;
	} = $props();

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
			meta: { namespace },
		})),
	}));
	const customResourcesByNamespace = $derived.by(() => {
		const rows = new Map<string, DiscoveredResourceKind[]>();
		if (!showCustomResources) return rows;
		for (const [index, query] of namespaceCustomResourceQueries.entries()) {
			const namespace = expandedNamespaces[index];
			if (namespace && query.data) rows.set(namespace, query.data as DiscoveredResourceKind[]);
		}
		return rows;
	});
	const customResourceErrorsByNamespace = $derived.by(() => {
		const errors = new Map<string, string>();
		if (!showCustomResources) return errors;
		for (const [index, query] of namespaceCustomResourceQueries.entries()) {
			const namespace = expandedNamespaces[index];
			if (!namespace || !query.isError) continue;
			errors.set(
				namespace,
				query.error instanceof Error ? query.error.message : String(query.error),
			);
		}
		return errors;
	});
	const pendingCustomResourceNamespaces = $derived.by(() => {
		const pending = new Set<string>();
		if (!showCustomResources) return pending;
		for (const [index, query] of namespaceCustomResourceQueries.entries()) {
			const namespace = expandedNamespaces[index];
			if (namespace && query.isPending) pending.add(namespace);
		}
		return pending;
	});
	function getLazyChildren(node: TreeNode) {
		if (node.id.type !== "namespace" || !node.id.namespace) return node.children;
		if (!showCustomResources) return node.children;
		const customResources = customResourcesByNamespace.get(node.id.namespace) ?? [];
		const children = buildNamespaceTreeNode(node.id.namespace, customResources).children ?? [];
		const error = customResourceErrorsByNamespace.get(node.id.namespace);
		if (error) {
			return children.concat({
				id: {
					type: "group",
					section: "namespaces",
					namespace: node.id.namespace,
					group: "custom-resources-error",
				},
				label: "Custom resources failed to load",
				description: error,
				disabled: true,
			});
		}
		if (!pendingCustomResourceNamespaces.has(node.id.namespace)) return children;
		return children.concat({
			id: {
				type: "group",
				section: "namespaces",
				namespace: node.id.namespace,
				group: "custom-resources-loading",
			},
			label: "Loading custom resources...",
			disabled: true,
		});
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
		showCustomResources,
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
