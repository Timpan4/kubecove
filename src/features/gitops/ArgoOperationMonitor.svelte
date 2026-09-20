<script lang="ts">
	import { createQuery, useQueryClient } from "@tanstack/svelte-query";
	import { createTauriClient, detectArgoCD, listArgoApplications } from "@/lib/tauri";
	import { queryKeys } from "@/lib/queryKeys";
	import { RESOURCE_RECOVERY_INTERVAL } from "@/lib/resource-watch";
	import type { WorkspaceReadContext } from "@/lib/workspaceReadContext";
	import { getArgoOperationTracker } from "./argo-operation-tracker";
	let { context }: { context: WorkspaceReadContext } = $props();
	const client = createTauriClient();
	const tracker = getArgoOperationTracker(useQueryClient());
	const detection = createQuery(() => ({
		queryKey: queryKeys.argoDetect(context.clusterContext, context.kubeconfigSourceKey),
		queryFn: () => detectArgoCD(client, context.clusterContext, context.kubeconfigSourceKey),
		enabled: context.sourceReady,
	}));
	createQuery(() => ({
		queryKey: queryKeys.argoApps(context.clusterContext, context.kubeconfigSourceKey),
		queryFn: () => listArgoApplications(client, context.clusterContext, context.kubeconfigSourceKey),
		enabled: context.sourceReady && detection.data === true,
		refetchInterval: RESOURCE_RECOVERY_INTERVAL,
		refetchIntervalInBackground: true,
	}));
	$effect(() => {
		if (!context.sourceReady) return;
		return tracker.activate({ context: context.clusterContext, workspaceId: context.workspaceId }, context.kubeconfigSourceKey);
	});
</script>
