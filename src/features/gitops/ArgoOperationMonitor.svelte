<script lang="ts">
	import { createQuery, useQueryClient } from "@tanstack/svelte-query";
	import { createTauriClient, detectArgoCD, listArgoApplications, refreshResourceCache } from "@/lib/tauri";
	import { queryKeys } from "@/lib/queryKeys";
	import { observeResourceScope } from "@/lib/resource-watch";
	import { createArgoListFreshness } from "./argo-application-freshness";
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
	$effect(() => {
		if (!context.sourceReady || detection.data !== true) return;
		const clusterContext = context.clusterContext;
		const source = context.kubeconfigSourceKey;
		const keys = [{ resourceKind: { kind: "Application", apiVersion: "argoproj.io/v1alpha1", plural: "applications", namespaced: true } }];
		const freshness = createArgoListFreshness((queryKey) => void queryClient.invalidateQueries({ queryKey }), source);
		const stop = observeResourceScope({
			client, clusterContext, keys, kubeconfigEnvVar: source,
			onState: () => {},
			onChange: freshness.handle,
			reload: async () => {
				await refreshResourceCache(client, clusterContext, keys, [], source);
				await queryClient.invalidateQueries({ queryKey: queryKeys.argoApps(clusterContext, source) }, { throwOnError: true });
			},
		});
		return () => { stop(); freshness.dispose(); };
	});
	$effect(() => {
		if (!context.sourceReady) return;
		return tracker.activate({ context: context.clusterContext, workspaceId: context.workspaceId }, context.kubeconfigSourceKey);
	});
</script>
