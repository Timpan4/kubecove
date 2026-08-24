<script lang="ts">
	import { createQuery, useQueryClient } from "@tanstack/svelte-query";
	import { onDestroy } from "svelte";
	import { workspaceStore } from "@/features/workspaces";
	import {
		createCancelScope,
		createFiniteReadCleanup,
		createFiniteReadRequest,
	} from "@/lib/finite-read-lifecycle";
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
	const queryClient = useQueryClient();
	const finiteReadCleanup = createFiniteReadCleanup(queryClient, (scope) =>
		cancelBackendRequests(client, scope),
	);
	const queryKey = $derived(
		queryKeys.rbacInspection(workspace.scope.clusterContext, kubeconfigSourceKey),
	);
	const cancelScope = $derived(
		createCancelScope("rbac-inspection", [
			workspace.id,
			workspace.scope.clusterContext,
			kubeconfigSourceKey ?? "default",
		]),
	);
	onDestroy(() => {
		finiteReadCleanup.schedule(cancelScope, queryKey);
		void cancelBackendRequests(client, "rbac-review").catch(() => {});
	});
	const rbacQuery = createQuery<RbacInspectionSummary>(() => ({
		queryKey,
		queryFn: () =>
			listRbacInspection(
				client,
				workspace.scope.clusterContext,
				kubeconfigSourceKey,
				createFiniteReadRequest(cancelScope, "rbac"),
			),
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
