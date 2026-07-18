<script lang="ts">
	import { createQuery } from "@tanstack/svelte-query";
	import { onDestroy } from "svelte";
	import { createCancelScope, createCancellableRequest } from "@/lib/cancellable-loads";
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
		verifierHandoff?: RbacVerifierHandoff;
		onVerifierHandoffConsumed?: () => void;
		onVerifierReturn?: () => void;
		verifierReturnLabel?: string;
	} = $props();

	const client = createTauriClient();
	function cancelScopeValue(): string {
		return createCancelScope("rbac-inspection", [
			workspace.id,
			workspace.scope.clusterContext,
			kubeconfigSourceKey ?? "default",
		]);
	}
	const cancelScope = cancelScopeValue();
	onDestroy(() => {
		void cancelBackendRequests(client, cancelScope).catch(() => {});
	});
	const rbacQuery = createQuery<RbacInspectionSummary>(() => ({
		queryKey: queryKeys.rbacInspection(
			workspace.scope.clusterContext,
			kubeconfigSourceKey,
		),
		queryFn: () =>
			listRbacInspection(
				client,
				workspace.scope.clusterContext,
				kubeconfigSourceKey,
				createCancellableRequest(cancelScope, "rbac"),
			),
		enabled: sourceReady,
		staleTime: 30_000,
	}));
	const view = $derived(selectedRbacView(selectedNode));
</script>

<RbacView query={rbacQuery} {view} warningSummary={rbacWarningSummary} {initialState} {onStateChange} {verifierHandoff} {onVerifierHandoffConsumed} {onVerifierReturn} {verifierReturnLabel} />
