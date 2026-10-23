import { diagnosticLog } from "@/lib/diagnostics";
import { finiteRead } from "@/lib/finite-read-lifecycle";
import { withForegroundLoad } from "@/lib/foreground-loading";
import { queryKeys } from "@/lib/queryKeys";
import {
	listResourceMetrics,
	listResourceScope,
	listResourceTopology,
	type TauriClient,
} from "@/lib/tauri";
import type {
	CancellableRequest,
	ResourceListRequest,
	ResourceScopeResult,
	TopologyMode,
} from "@/lib/types";
import {
	type FetchKey,
	isDiscoveredResourceKind,
	resourceKindLabel,
} from "./helpers";

function fetchKeyRequest({ kind, namespace }: FetchKey): ResourceListRequest {
	const normalizedNamespace = namespace === "" ? undefined : namespace;
	if (isDiscoveredResourceKind(kind)) {
		return { resourceKind: kind, namespace: normalizedNamespace };
	}
	return { kind, namespace: normalizedNamespace };
}

async function fetchResourcePage(
	client: TauriClient,
	{ clusterContext, fetchKeys, kubeconfigSourceKey }: ResourceScopeRead,
	cancellable: CancellableRequest,
): Promise<ResourceScopeResult> {
	const started = performance.now();
	diagnosticLog("resources.fetch.start", {
		cluster: clusterContext,
		fetches: fetchKeys.length,
		kinds: fetchKeys.map((key) => resourceKindLabel(key.kind)).join("|"),
	});
	const rows = await withForegroundLoad("resources", () =>
		listResourceScope(
			client,
			clusterContext,
			fetchKeys.map(fetchKeyRequest),
			kubeconfigSourceKey,
			cancellable,
		),
	);
	diagnosticLog("resources.fetch.done", {
		cluster: clusterContext,
		fetches: fetchKeys.length,
		rows: rows.resources.length,
		ms: Math.round(performance.now() - started),
	});
	return rows;
}

export interface ResourceScopeRead {
	clusterContext: string;
	fetchKeys: FetchKey[];
	kubeconfigSourceKey: string | undefined;
}

/** Resource list read shared by the Resource Browser and the command palette. */
export function resourceScopeQuery(client: TauriClient, read: ResourceScopeRead) {
	return {
		...finiteRead({
			queryKey: queryKeys.resources(read.clusterContext, read.fetchKeys, read.kubeconfigSourceKey),
			label: "resources.scope",
			read: (cancellable) => fetchResourcePage(client, read, cancellable),
		}),
		staleTime: 30_000,
	};
}

export interface ResourceTopologyRead {
	clusterContext: string;
	namespaces: string[];
	mode: TopologyMode;
	kubeconfigSourceKey: string | undefined;
	customResourceKinds?: string;
}

/** Topology read shared by the Resource Browser map and Incident Workbench ownership. */
export function resourceTopologyQuery(client: TauriClient, read: ResourceTopologyRead) {
	const { clusterContext, namespaces, mode, kubeconfigSourceKey } = read;
	return {
		...finiteRead({
			queryKey: queryKeys.resourceTopology(
				clusterContext,
				namespaces,
				mode,
				kubeconfigSourceKey,
				read.customResourceKinds,
			),
			label: "resources.topology",
			read: (cancellable) =>
				withForegroundLoad("resource-topology", () =>
					listResourceTopology(client, clusterContext, namespaces, mode, kubeconfigSourceKey, cancellable),
				),
		}),
		staleTime: 30_000,
		retry: false,
	};
}

export function resourceMetricsQuery(
	client: TauriClient,
	{ clusterContext, namespaces, kubeconfigSourceKey }: Omit<ResourceTopologyRead, "mode">,
) {
	return {
		...finiteRead({
			queryKey: queryKeys.resourceMetrics(clusterContext, namespaces, kubeconfigSourceKey),
			label: "resources.metrics",
			read: (cancellable) =>
				listResourceMetrics(client, clusterContext, namespaces, kubeconfigSourceKey, cancellable),
		}),
		staleTime: 30_000,
		retry: false,
	};
}
