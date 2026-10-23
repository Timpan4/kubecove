<script lang="ts">
	import { createQuery, useQueryClient } from "@tanstack/svelte-query";
	import { argoApplicationWatchKey } from "@/lib/argo-application-inspection";
	import { liveViewRefresh } from "@/lib/live-view-refresh.svelte";
	import { createTauriClient, detectArgoCD, listArgoApplications } from "@/lib/tauri";
	import { queryKeys, queryMatchers } from "@/lib/queryKeys";
	import type { WorkspaceReadContext } from "@/lib/workspaceReadContext";
	import { getArgoOperationTracker } from "./argo-operation-tracker";
	let { context }: { context: WorkspaceReadContext } = $props();
	const client = createTauriClient();
	const queryClient = useQueryClient();
	const tracker = getArgoOperationTracker(queryClient);
	const detection = createQuery(() => ({
		queryKey: queryKeys.argoDetect(context.clusterContext, context.kubeconfigSourceKey),
		queryFn: () => detectArgoCD(client, context.clusterContext, context.kubeconfigSourceKey),
		enabled: context.sourceReady,
	}));
	createQuery(() => ({
		queryKey: queryKeys.argoApps(context.clusterContext, context.kubeconfigSourceKey),
		queryFn: () => listArgoApplications(client, context.clusterContext, context.kubeconfigSourceKey),
		enabled: context.sourceReady && detection.data === true,
	}));
	// Keeps the Application list fresh across navigation; recovery rereads only that list.
	liveViewRefresh(client, () => {
		if (!context.sourceReady || detection.data !== true) return null;
		const keys = [argoApplicationWatchKey()];
		const match = queryMatchers.exact(queryKeys.argoApps(context.clusterContext, context.kubeconfigSourceKey));
		return {
			clusterContext: context.clusterContext,
			kubeconfigSourceKey: context.kubeconfigSourceKey,
			namespaces: [],
			keys,
			reads: [{ keys, match }],
			refreshScope: "reads",
		};
	});
	$effect(() => {
		if (!context.sourceReady) return;
		return tracker.activate({ context: context.clusterContext, workspaceId: context.workspaceId }, context.kubeconfigSourceKey);
	});
</script>
