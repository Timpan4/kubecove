import { queryKeys } from "@/lib/queryKeys";
import type { TauriClient } from "@/lib/tauri";
import type { TopologyMode } from "@/lib/types";
import { type FetchKey, resourceKindFetchKey } from "./helpers";
import { resourceMetricsQuery, resourceScopeQuery, resourceTopologyQuery } from "./query";

export interface ResourceBrowserReadSpecInput {
	client: TauriClient;
	clusterContext: string;
	kubeconfigSourceKey: string | undefined;
	fetchKeys: FetchKey[];
	namespaces: string[];
	topologyMode: TopologyMode;
	mapPanelOpen: boolean;
	sourceReady: boolean;
	customResourcesEnabled: boolean;
}

// Scope and map changes keep the previous rows and graph visible while the next read loads.
const keepPreviousData = <T>(previousData: T | undefined) => previousData;

/** Immutable identities, enablement, and finite-read options for ResourceBrowser-owned reads. */
export function buildResourceBrowserReadSpecs({
	client,
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
	const customResourceKinds = [...new Set(
		fetchKeys
			.filter((key) => key.kind instanceof Object)
			.map((key) => resourceKindFetchKey(key.kind)),
	)]
		.sort()
		.join(",");

	return {
		topologyNamespaces,
		namespacesQueryKey: queryKeys.namespaces(clusterContext, kubeconfigSourceKey),
		resourceKindsQueryKey: queryKeys.resourceKinds(
			clusterContext,
			kubeconfigSourceKey,
		),
		// Custom kinds refetch the map but keep its fitted viewport.
		topologyFitViewKey: JSON.stringify([
			clusterContext,
			topologyNamespaces,
			topologyMode,
			kubeconfigSourceKey ?? "",
		]),
		namespacesEnabled: Boolean(clusterContext) && sourceReady,
		resourceKindsEnabled:
			customResourcesEnabled && Boolean(clusterContext) && sourceReady,
		resources: {
			...resourceScopeQuery(client, { clusterContext, fetchKeys, kubeconfigSourceKey }),
			enabled: Boolean(clusterContext) && fetchKeys.length > 0 && sourceReady,
			placeholderData: keepPreviousData,
		},
		topology: {
			...resourceTopologyQuery(client, {
				clusterContext,
				namespaces: topologyNamespaces,
				mode: topologyMode,
				kubeconfigSourceKey,
				customResourceKinds,
			}),
			enabled: Boolean(clusterContext && mapPanelOpen) && sourceReady,
			placeholderData: keepPreviousData,
		},
		// The Browser enables metrics after rows and the map settle.
		metrics: resourceMetricsQuery(client, {
			clusterContext,
			namespaces: topologyNamespaces,
			kubeconfigSourceKey,
		}),
	};
}
