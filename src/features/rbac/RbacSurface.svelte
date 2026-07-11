<script lang="ts">
	import { createQuery } from "@tanstack/svelte-query";
	import { queryKeys } from "@/lib/queryKeys";
	import { createTauriClient, listRbacInspection } from "@/lib/tauri";
	import type { RbacInspectionSummary } from "@/lib/types";
	import type { TreeNodeId } from "@/lib/tree-nav";
	import type { SavedWorkspace } from "@/lib/workspace-model";
	import RbacView from "./RbacView.svelte";
	import {
		buildRbacStats,
		buildRbacTable,
		selectedRbacView,
		rbacWarningSummary,
	} from "./surfaceModel";

	let {
		workspace,
		sourceReady,
		kubeconfigSourceKey,
		selectedNode,
	}: {
		workspace: SavedWorkspace;
		sourceReady: boolean;
		kubeconfigSourceKey?: string;
		selectedNode: TreeNodeId | null;
	} = $props();

	const client = createTauriClient();
	const rbacQuery = createQuery<RbacInspectionSummary>(() => ({
		queryKey: queryKeys.rbacInspection(
			workspace.scope.clusterContext,
			workspace.scope.namespaces,
			kubeconfigSourceKey,
		),
		queryFn: () =>
			listRbacInspection(
				client,
				workspace.scope.clusterContext,
				workspace.scope.namespaces,
				kubeconfigSourceKey,
			),
		enabled: sourceReady,
		staleTime: 30_000,
	}));
	const view = $derived(selectedRbacView(selectedNode));
	const table = $derived(rbacQuery.data ? buildRbacTable(rbacQuery.data, view) : null);
	const stats = $derived(rbacQuery.data ? buildRbacStats(rbacQuery.data) : []);
</script>

<RbacView query={rbacQuery} {stats} {table} {view} warningSummary={rbacWarningSummary} />
