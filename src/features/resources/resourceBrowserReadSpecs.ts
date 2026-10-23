import {
	argoApplicationReads,
	argoApplicationWatchKey,
	type ArgoApplicationTarget,
} from "@/lib/argo-application-inspection";
import type { LiveRead, LiveView } from "@/lib/live-view-refresh";
import { queryKeys, queryMatchers } from "@/lib/queryKeys";
import type { TauriClient } from "@/lib/tauri";
import type { TopologyMode } from "@/lib/types";
import type { WorkspaceReadContext } from "@/lib/workspaceReadContext";
import {
	type FetchKey,
	mergeWatchKeys,
	resourceKindFetchKey,
	topologyWatchKeys,
	watchKeysFromFetchKeys,
} from "./helpers";
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

/**
 * The watches that keep the Browser's rows, map, and focused Application fresh.
 * A closed map is not watched, so every change marks its topology stale instead.
 */
export function resourceBrowserLiveView(
	readContext: Pick<WorkspaceReadContext, "workspaceId" | "clusterContext" | "kubeconfigSourceKey" | "sourceReady">,
	specs: Pick<ReturnType<typeof buildResourceBrowserReadSpecs>, "resources" | "topology" | "topologyNamespaces">,
	{
		fetchKeys,
		namespaces,
		mapPanelOpen,
		focusedApplication,
	}: {
		fetchKeys: FetchKey[];
		namespaces: string[];
		mapPanelOpen: boolean;
		focusedApplication: ArgoApplicationTarget | null;
	},
): LiveView | null {
	const { clusterContext, kubeconfigSourceKey } = readContext;
	if (!readContext.sourceReady || !clusterContext) return null;
	const view = { clusterContext, kubeconfigSourceKey, namespaces, refreshScope: "view" } as const;
	if (fetchKeys.length === 0) return { ...view, keys: [], reads: [] };
	const resourceKeys = watchKeysFromFetchKeys(fetchKeys);
	const mapKeys = mapPanelOpen
		? mergeWatchKeys(topologyWatchKeys(specs.topologyNamespaces), resourceKeys.filter((key) => Boolean(key.resourceKind.group)))
		: [];
	const argoKeys = focusedApplication ? [argoApplicationWatchKey(focusedApplication.namespace)] : [];
	const keys = mergeWatchKeys(resourceKeys, mapKeys, argoKeys);
	const topology = queryMatchers.exact(specs.topology.queryKey);
	const reads: LiveRead[] = [
		{ keys: resourceKeys, match: queryMatchers.exact(specs.resources.queryKey) },
		mapPanelOpen ? { keys: mapKeys, match: topology } : { keys, match: topology, refetch: false },
	];
	if (focusedApplication) {
		reads.push({ keys: [...resourceKeys, ...argoKeys], match: argoApplicationReads(readContext, focusedApplication) });
	}
	return { ...view, keys, reads };
}
