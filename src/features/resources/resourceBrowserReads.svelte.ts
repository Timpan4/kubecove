import { createQueries, createQuery } from "@tanstack/svelte-query";
import type { ArgoApplicationTarget } from "@/lib/argo-application-inspection";
import { diagnosticLog } from "@/lib/diagnostics";
import type { KindFetchKey } from "@/lib/kind-identity";
import { liveViewRefresh } from "@/lib/live-view-refresh.svelte";
import { cancelledQueryRetry } from "@/lib/query-retry";
import { describeMetricsAvailability, resourceMetricIndex } from "@/lib/resource-metrics";
import { listNamespaces, listResourceKinds, type TauriClient } from "@/lib/tauri";
import type {
	DiscoveredResourceKind,
	NamespaceSummary,
	ResourceScopeResult,
	ResourceTopology,
	TopologyMode,
} from "@/lib/types";
import type { WorkspaceReadContext } from "@/lib/workspaceReadContext";
import { mergeContextResults } from "./contextResults";
import { buildResourceBrowserReadSpecs, resourceBrowserLiveView } from "./resourceBrowserReadSpecs";

const BACKGROUND_METRICS_DELAY_MS = 1_500;

export interface ResourceBrowserReadInput {
	readContext: WorkspaceReadContext;
	fetchKeys: KindFetchKey[];
	namespaces: string[];
	topologyMode: TopologyMode;
	mapPanelOpen: boolean;
	customResourcesEnabled: boolean;
	focusedApplication: ArgoApplicationTarget | null;
}

/**
 * The Resource Browser's reads: scope rows, namespaces, kinds and metrics for every
 * workspace context, the ownership map for the one map context, and one live view
 * per context. A focused Argo CD Application reads only the primary context.
 */
export function resourceBrowserReads(client: TauriClient, input: () => ResourceBrowserReadInput) {
	const current = $derived(input());
	const primary = $derived(current.readContext.clusterContext);
	const sourceReady = $derived(current.readContext.sourceReady);
	const kubeconfigSourceKey = $derived(current.readContext.kubeconfigSourceKey);
	const contexts = $derived(
		current.focusedApplication || current.readContext.clusterContexts.length === 0
			? (primary ? [primary] : [])
			: [...current.readContext.clusterContexts],
	);
	let chosenMapContext = $state<string | null>(null);
	const mapContext = $derived(chosenMapContext !== null && contexts.includes(chosenMapContext) ? chosenMapContext : primary);
	const specsFor = (clusterContext: string) => buildResourceBrowserReadSpecs({
		client,
		clusterContext,
		kubeconfigSourceKey,
		fetchKeys: current.fetchKeys,
		namespaces: current.namespaces,
		topologyMode: current.topologyMode,
		mapPanelOpen: current.mapPanelOpen,
		sourceReady,
		customResourcesEnabled: current.customResourcesEnabled,
	});
	const specs = $derived(contexts.map((clusterContext) => ({ clusterContext, specs: specsFor(clusterContext) })));
	const mapSpecs = $derived(specsFor(mapContext));

	// Each context keeps its own last settled rows, shown while its next read loads.
	// New query keys get new observers, so TanStack's previous data would be empty.
	const settledRows = new Map<string, ResourceScopeResult>();
	const scopeQueries = createQueries(() => ({
		queries: specs.map(({ clusterContext, specs: spec }) => ({
			...spec.resources,
			placeholderData: (previous: ResourceScopeResult | undefined) => previous ?? settledRows.get(clusterContext),
		})),
	}));
	const scope = $derived(mergeContextResults(scopeQueries.map((query, index) => ({
		clusterContext: specs[index]?.clusterContext ?? "",
		data: query.data,
		error: query.error,
		isError: query.isError,
		isPending: query.isPending,
		isPlaceholderData: query.isPlaceholderData,
	}))));
	$effect(() => {
		const settled = new Map<string, ResourceScopeResult>();
		scopeQueries.forEach((query, index) => {
			const clusterContext = specs[index]?.clusterContext;
			if (clusterContext && query.isSuccess && !query.isPlaceholderData) settled.set(clusterContext, query.data);
		});
		for (const clusterContext of settledRows.keys()) if (!contexts.includes(clusterContext)) settledRows.delete(clusterContext);
		for (const [clusterContext, data] of settled) settledRows.set(clusterContext, data);
	});

	const namespaceQueries = createQueries(() => ({
		queries: specs.map(({ clusterContext, specs: spec }) => ({
			queryKey: spec.namespacesQueryKey,
			queryFn: () => listNamespaces(client, clusterContext, kubeconfigSourceKey),
			enabled: spec.namespacesEnabled,
			staleTime: 30_000,
			retry: false,
		})),
	}));
	const kindQueries = createQueries(() => ({
		queries: specs.map(({ clusterContext, specs: spec }) => ({
			queryKey: spec.resourceKindsQueryKey,
			queryFn: () => listResourceKinds(client, clusterContext, kubeconfigSourceKey),
			enabled: spec.resourceKindsEnabled,
			staleTime: 30_000,
			retry: cancelledQueryRetry,
		})),
	}));
	const namespaces = $derived(unionBy(namespaceQueries.map((query) => query.data ?? []), (namespace: NamespaceSummary) => namespace.name));
	const kinds = $derived(unionBy(kindQueries.map((query) => query.data ?? []), (kind: DiscoveredResourceKind) =>
		`${kind.group}/${kind.version}/${kind.plural}/${kind.kind}`));

	// A changed map context does not show the previous context's graph while its own loads.
	let settledMapContext: string | null = null;
	const topology = createQuery<ResourceTopology>(() => {
		const context = mapContext;
		return {
			...mapSpecs.topology,
			placeholderData: (previous: ResourceTopology | undefined) => (settledMapContext === context ? previous : undefined),
		};
	});
	$effect(() => {
		if (topology.isSuccess && !topology.isPlaceholderData) settledMapContext = mapContext;
	});

	let metricsReady = $state(false);
	let metricsScopeKey = "";
	$effect(() => {
		const key = JSON.stringify(specs.map(({ specs: spec }) => spec.metrics.queryKey));
		if (metricsScopeKey === key) return;
		metricsScopeKey = key;
		metricsReady = false;
	});
	$effect(() => {
		const rows = scope.data?.resources.length ?? 0;
		const mapOpen = current.mapPanelOpen;
		if (metricsReady || contexts.length === 0 || !scope.isSuccess || scope.isPlaceholderData
			|| (mapOpen && topology.isPending) || !globalThis.window) return;
		diagnosticLog("resources.metrics.defer", { ms: BACKGROUND_METRICS_DELAY_MS, rows, mapOpen });
		const timer = window.setTimeout(() => {
			diagnosticLog("resources.metrics.enable", { rows, mapOpen });
			metricsReady = true;
		}, BACKGROUND_METRICS_DELAY_MS);
		return () => window.clearTimeout(timer);
	});
	const metricsQueries = createQueries(() => ({
		queries: specs.map(({ specs: spec }) => ({ ...spec.metrics, enabled: metricsReady })),
	}));
	const metricsIndex = $derived(resourceMetricIndex(metricsQueries.map((query, index) => ({
		clusterContext: specs[index]?.clusterContext ?? "",
		summary: query.data,
	}))));
	const metricsMessage = $derived(
		metricsQueries.some((query) => query.isError)
			? "metrics API unavailable"
			: metricsQueries.map((query) => describeMetricsAvailability(query.data?.availability)).find(Boolean) ?? null,
	);

	const live = liveViewRefresh(client, () => specs.flatMap(({ clusterContext, specs: spec }) => {
		const view = resourceBrowserLiveView(
			{ workspaceId: current.readContext.workspaceId, clusterContext, kubeconfigSourceKey, sourceReady },
			spec,
			{
				fetchKeys: current.fetchKeys,
				namespaces: current.namespaces,
				mapPanelOpen: current.mapPanelOpen && clusterContext === mapContext,
				focusedApplication: clusterContext === primary ? current.focusedApplication : null,
				shownVersions: scope.versions.get(clusterContext) ?? null,
			},
		);
		return view ? [view] : [];
	}));

	return {
		/** Read contexts in workspace rank order. */
		get contexts() { return contexts; },
		get mapContext() { return mapContext; },
		selectMapContext(clusterContext: string) { chosenMapContext = clusterContext; },
		get scope() { return scope; },
		get namespaces() { return namespaces; },
		get namespacesReady() { return namespaceQueries.length > 0 && namespaceQueries.every((query) => query.isSuccess); },
		get kinds() { return kinds; },
		get kindsReady() { return kindQueries.length > 0 && kindQueries.every((query) => query.isSuccess); },
		get topology() { return topology; },
		get topologyFitViewKey() { return mapSpecs.topologyFitViewKey; },
		get metricsIndex() { return metricsIndex; },
		get metricsMessage() { return metricsMessage; },
		live,
	};
}

/** Items of every list in order, keeping the first of each identity. */
function unionBy<T>(lists: readonly (readonly T[])[], identity: (item: T) => string): T[] {
	if (lists.length === 1) return [...lists[0]];
	const seen = new Map<string, T>();
	for (const list of lists) for (const item of list) if (!seen.has(identity(item))) seen.set(identity(item), item);
	return [...seen.values()];
}
