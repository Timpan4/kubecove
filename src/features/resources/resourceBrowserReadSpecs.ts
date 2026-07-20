import { createCancelScope } from "@/lib/cancellable-loads";
import { queryKeys } from "@/lib/queryKeys";
import type { TopologyMode } from "@/lib/types";
import { type FetchKey, resourceKindFetchKey } from "./helpers";

export interface ResourceBrowserReadSpecInput {
	clusterContext: string;
	kubeconfigSourceKey: string | undefined;
	fetchKeys: FetchKey[];
	namespaces: string[];
	topologyMode: TopologyMode;
	mapPanelOpen: boolean;
	sourceReady: boolean;
	customResourcesEnabled: boolean;
}

/** Immutable identities and enablement for ResourceBrowser-owned reads. */
export function buildResourceBrowserReadSpecs({
	clusterContext,
	kubeconfigSourceKey,
	fetchKeys,
	namespaces,
	topologyMode,
	mapPanelOpen,
	sourceReady,
	customResourcesEnabled,
}: ResourceBrowserReadSpecInput) {
	const topologyNamespaces = [...new Set(namespaces)].sort();
	const topologyCustomResourceKey = [...new Set(
		fetchKeys
			.filter((key) => typeof key.kind !== "string")
			.map((key) => resourceKindFetchKey(key.kind)),
	)]
		.toSorted()
		.join(",");
	const resourceQueryKey = queryKeys.resources(
		clusterContext,
		fetchKeys,
		kubeconfigSourceKey,
	);
	const topologyBaseQueryKey = queryKeys.resourceTopology(
		clusterContext,
		topologyNamespaces,
		topologyMode,
		kubeconfigSourceKey,
	);
	const topologyQueryKey = [...topologyBaseQueryKey, topologyCustomResourceKey] as const;
	const metricsQueryKey = queryKeys.resourceMetrics(
		clusterContext,
		topologyNamespaces,
		kubeconfigSourceKey,
	);

	return {
		topologyNamespaces,
		namespacesQueryKey: queryKeys.namespaces(clusterContext, kubeconfigSourceKey),
		resourceKindsQueryKey: queryKeys.resourceKinds(
			clusterContext,
			kubeconfigSourceKey,
		),
		resourceQueryKey,
		topologyBaseQueryKey,
		topologyQueryKey,
		metricsQueryKey,
		resourceCancelScope: createCancelScope("resources", resourceQueryKey),
		topologyCancelScope: createCancelScope("resource-topology", topologyQueryKey),
		metricsCancelScope: createCancelScope("resource-metrics", metricsQueryKey),
		namespacesEnabled: Boolean(clusterContext) && sourceReady,
		resourceKindsEnabled:
			customResourcesEnabled && Boolean(clusterContext) && sourceReady,
		resourcesEnabled:
			Boolean(clusterContext) && fetchKeys.length > 0 && sourceReady,
		topologyEnabled: Boolean(clusterContext && mapPanelOpen),
	};
}
