<script lang="ts">
	import { createQuery } from "@tanstack/svelte-query";
	import { onDestroy } from "svelte";
	import { workspaceStore } from "@/features/workspaces";
	import { finiteRead } from "@/lib/finite-read-lifecycle";
	import { queryKeys } from "@/lib/queryKeys";
	import { cancelBackendRequests, createTauriClient, listRbacInspection } from "@/lib/tauri";
	import type { RbacInspectionSummary } from "@/lib/types";
	import type { TreeNodeId } from "@/lib/tree-nav";
	import type { SavedWorkspace } from "@/lib/workspace-model";
	import RbacView from "./RbacView.svelte";
	import { selectedRbacView, rbacWarningSummary } from "./surfaceModel";
	import type { RbacCockpitState, RbacVerifierHandoff } from "./cockpitModel";

	let {
		workspace,
		sourceReady,
		kubeconfigSourceKey,
		selectedNode,
		initialState,
		onStateChange,
		onViewChange,
		verifierHandoff,
		onVerifierHandoffConsumed,
		onVerifierReturn,
		verifierReturnLabel,
	}: {
		workspace: SavedWorkspace;
		sourceReady: boolean;
		kubeconfigSourceKey?: string;
		selectedNode: TreeNodeId | null;
		initialState?: RbacCockpitState;
		onStateChange?: (state: RbacCockpitState) => void;
		onViewChange?: (view: import("./surfaceModel").RbacView) => void;
		verifierHandoff?: RbacVerifierHandoff;
		onVerifierHandoffConsumed?: () => void;
		onVerifierReturn?: () => void;
		verifierReturnLabel?: string;
	} = $props();

	const client = createTauriClient();
	// Permission verification reviews run outside TanStack Query under their own scope.
	onDestroy(() => {
		void cancelBackendRequests(client, "rbac-review").catch(() => {});
	});
	const rbacQuery = createQuery<RbacInspectionSummary>(() => ({
		...finiteRead({
			queryKey: queryKeys.rbacInspection(workspace.scope.clusterContext, kubeconfigSourceKey),
			label: "rbac.inspection",
			read: (cancellable) =>
				listRbacInspection(client, workspace.scope.clusterContext, kubeconfigSourceKey, cancellable),
		}),
		enabled: sourceReady,
		staleTime: 30_000,
	}));
	const view = $derived(selectedRbacView(selectedNode));
</script>

<RbacView
	query={rbacQuery}
	{view}
	warningSummary={rbacWarningSummary}
	{initialState}
	{onStateChange}
	{onViewChange}
	{verifierHandoff}
	{onVerifierHandoffConsumed}
	{onVerifierReturn}
	{verifierReturnLabel}
	reviewRecords={workspace.rbacReviews ?? []}
	onReviewRecordsChange={(records) => workspaceStore.setRbacReviews(workspace.id, records)}
/>
