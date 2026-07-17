import {
	QueryClient,
	type TimeoutCallback,
	timeoutManager,
} from "@tanstack/svelte-query";
import {
	buildNamespaceTreeNode,
	buildShallowNamespaceTreeNode,
} from "../src/components/sidebar-tree-helpers";
import {
	applyFlowTopologySelectionWithIndex,
	buildFlowTopology,
	buildFlowTopologyLayout,
	buildFlowTopologySelectionIndex,
	resourceTopologyNodeId,
} from "../src/features/resources/topology-implementation";
import {
	configureLargeQueryRetention,
	LARGE_QUERY_ROOTS,
} from "../src/lib/query-retention";
import {
	mergeResourceMetrics,
	mergeTopologyMetrics,
	resourceMetricIndex,
} from "../src/lib/resource-metrics";
import type {
	DiscoveredResourceKind,
	ResourceMetricSummary,
	ResourceMetricsSummary,
	ResourceSummary,
	ResourceTopology,
	TopologyNode,
} from "../src/lib/types";

const namespaces = Array.from({ length: 1_000 }, (_, index) => `namespace-${index}`);
const extraKinds: DiscoveredResourceKind[] = Array.from({ length: 100 }, (_, index) => ({
	group: "example.com",
	version: "v1",
	apiVersion: "example.com/v1",
	kind: `Widget${index}`,
	plural: `widgets${index}`,
	namespaced: true,
}));

function forceGc() {
	Bun.gc(true);
}

function rssBytes() {
	forceGc();
	return process.memoryUsage().rss;
}

function heapBytes() {
	forceGc();
	return process.memoryUsage().heapUsed;
}

function formatMiB(bytes: number) {
	return Number((bytes / 1024 / 1024).toFixed(2));
}

function memorySample() {
	return {
		rss: rssBytes(),
		heap: heapBytes(),
	};
}

function memoryDelta(after: ReturnType<typeof memorySample>, before: ReturnType<typeof memorySample>) {
	return {
		rssMiB: formatMiB(after.rss - before.rss),
		heapMiB: formatMiB(after.heap - before.heap),
	};
}

function countTreeNodes(nodes: unknown): number {
	if (!Array.isArray(nodes)) return 0;
	let count = 0;
	const stack = [...nodes] as Array<{ children?: unknown }>;
	while (stack.length > 0) {
		const node = stack.pop();
		if (!node) continue;
		count += 1;
		if (Array.isArray(node.children)) {
			stack.push(...(node.children as Array<{ children?: unknown }>));
		}
	}
	return count;
}

class VirtualTimeouts {
	#now = 0;
	#nextId = 0;
	#timers = new Map<number, { callback: TimeoutCallback; dueAt: number }>();

	setTimeout = (callback: TimeoutCallback, delay: number): number => {
		const id = this.#nextId++;
		this.#timers.set(id, { callback, dueAt: this.#now + delay });
		return id;
	};

	clearTimeout = (id: number | undefined): void => {
		if (id !== undefined) this.#timers.delete(id);
	};

	setInterval = this.setTimeout;
	clearInterval = this.clearTimeout;

	advanceBy(milliseconds: number): void {
		this.#now += milliseconds;
		for (const [id, timer] of [...this.#timers]) {
			if (timer.dueAt > this.#now) continue;
			this.#timers.delete(id);
			timer.callback();
		}
	}
}

function summary(kind: string, name: string, namespace: string): ResourceSummary {
	return {
		cluster: "prod",
		kind,
		name,
		namespace,
		age: "1m",
		status: "Running",
		ready: "True",
		apiVersion: kind === "Pod" ? "v1" : "apps/v1",
	};
}

function topologyNode(kind: string, name: string, namespace: string): TopologyNode {
	return {
		id: resourceTopologyNodeId(
			"prod",
			kind === "Pod" ? "v1" : "apps/v1",
			kind,
			namespace,
			name,
		),
		kind,
		name,
		namespace,
		status: "Running",
		health: "healthy",
		selectable: true,
		summary: summary(kind, name, namespace),
	};
}

function buildTopology(apps: number): ResourceTopology {
	const nodes: TopologyNode[] = [];
	const edges: ResourceTopology["edges"] = [];
	for (let index = 0; index < apps; index += 1) {
		const namespace = `namespace-${index % 100}`;
		const deployment = topologyNode("Deployment", `api-${index}`, namespace);
		const pod = topologyNode("Pod", `api-${index}-pod`, namespace);
		nodes.push(deployment, pod);
		edges.push({
			id: `edge-${index}`,
			source: deployment.id,
			target: pod.id,
			relation: "owns",
		});
	}
	return { nodes, edges, warnings: [] };
}

function buildMetrics(total: number): ResourceMetricsSummary {
	const pods: ResourceMetricSummary[] = [];
	const nodes: ResourceMetricSummary[] = [];
	const workloads: ResourceMetricSummary[] = [];
	for (let index = 0; index < total; index += 1) {
		const kind = index % 3 === 0 ? "Pod" : index % 3 === 1 ? "Node" : "Deployment";
		const metric: ResourceMetricSummary = {
			kind,
			cluster: "prod",
			name: `${kind.toLowerCase()}-${index}`,
			namespace: kind === "Node" ? null : `namespace-${index % 100}`,
			cpuMillicores: index % 1_000,
			memoryBytes: (index % 512) * 1024 * 1024,
			sourcePods: [],
		};
		if (kind === "Pod") pods.push(metric);
		else if (kind === "Node") nodes.push(metric);
		else workloads.push(metric);
	}
	return {
		cluster: "prod",
		availability: { status: "available" },
		pods,
		nodes,
		workloads,
		warnings: [],
	};
}

function percentile(values: number[], percentile: number) {
	const sorted = [...values].sort((left, right) => left - right);
	return sorted[Math.floor((sorted.length - 1) * percentile)] ?? 0;
}

const sidebarStartMemory = memorySample();
let started = performance.now();
const eagerNamespaceNodes = namespaces.map((namespace) =>
	buildNamespaceTreeNode(namespace, extraKinds),
);
const eagerSidebarMs = performance.now() - started;
const eagerSidebarMemory = memorySample();
const eagerNodeCount = countTreeNodes(eagerNamespaceNodes);

started = performance.now();
const shallowNamespaceNodes = namespaces.map(buildShallowNamespaceTreeNode);
const shallowSidebarMs = performance.now() - started;
const shallowSidebarMemory = memorySample();
const shallowNodeCount = countTreeNodes(shallowNamespaceNodes);

const topology = buildTopology(2_000);
const selectedIds = topology.nodes
	.filter((node) => node.kind === "Pod")
	.slice(0, 100)
	.map((node) => node.id);

started = performance.now();
for (const selectedId of selectedIds) {
	buildFlowTopology(topology, selectedId);
}
const rebuildSelectionMs = performance.now() - started;
const afterRebuildSelectionMemory = memorySample();

started = performance.now();
const layout = buildFlowTopologyLayout(topology, null);
const layoutBuildMs = performance.now() - started;
const selectionIndex = buildFlowTopologySelectionIndex(topology);
const afterLayoutMemory = memorySample();

started = performance.now();
for (const selectedId of selectedIds) {
	applyFlowTopologySelectionWithIndex(layout, selectionIndex, selectedId);
}
const indexedSelectionMs = performance.now() - started;
const afterIndexedSelectionMemory = memorySample();

const metricTotal = 10_000;
const metrics = buildMetrics(metricTotal);
const metricResources = Array.from({ length: metricTotal }, (_, index) => {
	const kind = index % 3 === 0 ? "Pod" : index % 3 === 1 ? "Node" : "Deployment";
	return summary(kind, `${kind.toLowerCase()}-${index}`, kind === "Node" ? "" : `namespace-${index % 100}`);
});
const metricTopology: ResourceTopology = {
	nodes: metricResources.map((summary, index) => ({
		id: `metric-node-${index}`,
		kind: summary.kind,
		name: summary.name,
		namespace: summary.namespace,
		health: "healthy",
		selectable: true,
		summary,
	})),
	edges: [],
	warnings: [],
};
const metricSamples = { duplicateIndexMs: [] as number[], sharedIndexMs: [] as number[] };
for (let index = 0; index < 20; index += 1) {
	started = performance.now();
	mergeResourceMetrics(metricResources, metrics);
	mergeTopologyMetrics(metricTopology, metrics);
	metricSamples.duplicateIndexMs.push(performance.now() - started);

	started = performance.now();
	const metricIndex = resourceMetricIndex(metrics);
	mergeResourceMetrics(metricResources, metrics, metricIndex);
	mergeTopologyMetrics(metricTopology, metrics, metricIndex);
	metricSamples.sharedIndexMs.push(performance.now() - started);
}

const queryTimeouts = new VirtualTimeouts();
timeoutManager.setTimeoutProvider(queryTimeouts);
const queryClient = new QueryClient();
configureLargeQueryRetention(queryClient);
for (const root of LARGE_QUERY_ROOTS) {
	queryClient.setQueryData(
		[root, "memory-measurement"],
		Array.from({ length: 25_000 }, (_, index) => ({
			id: `${root}-${index}-${"x".repeat(64)}`,
			name: `resource-${index}`,
			status: index % 2 === 0 ? "Ready" : "Pending",
		})),
	);
}
const retainedPayloadRows = LARGE_QUERY_ROOTS.reduce(
	(total, root) =>
		total +
		(queryClient.getQueryData<unknown[]>([root, "memory-measurement"])?.length ?? 0),
	0,
);
const retainedPayloadBytes = LARGE_QUERY_ROOTS.reduce(
	(total, root) =>
		total +
		JSON.stringify(queryClient.getQueryData([root, "memory-measurement"])).length,
	0,
);
queryTimeouts.advanceBy(89_999);
const retainedQueriesBeforeExpiry = queryClient.getQueryCache().getAll().length;
queryTimeouts.advanceBy(1);
const retainedQueriesAfterExpiry = queryClient.getQueryCache().getAll().length;

console.log(
	JSON.stringify(
		{
			sidebar: {
				namespaces: namespaces.length,
				extraNamespacedKinds: extraKinds.length,
				eagerNodeCount,
				shallowNodeCount,
				nodeReduction: Number((eagerNodeCount / shallowNodeCount).toFixed(2)),
				eagerBuildMs: Number(eagerSidebarMs.toFixed(2)),
				shallowBuildMs: Number(shallowSidebarMs.toFixed(2)),
				buildSpeedup: Number((eagerSidebarMs / shallowSidebarMs).toFixed(2)),
				eagerIncrementalMemory: memoryDelta(eagerSidebarMemory, sidebarStartMemory),
				shallowIncrementalMemory: memoryDelta(
					shallowSidebarMemory,
					eagerSidebarMemory,
				),
			},
			topologySelection: {
				apps: 2_000,
				nodes: topology.nodes.length,
				edges: topology.edges.length,
				selections: selectedIds.length,
				rebuildEverySelectionMs: Number(rebuildSelectionMs.toFixed(2)),
				layoutBuildOnceMs: Number(layoutBuildMs.toFixed(2)),
				applySelectionIndexedMs: Number(indexedSelectionMs.toFixed(2)),
				selectionSpeedup: Number((rebuildSelectionMs / indexedSelectionMs).toFixed(2)),
				rebuildSelectionMemory: memoryDelta(afterRebuildSelectionMemory, shallowSidebarMemory),
				layoutAndIndexMemory: memoryDelta(afterLayoutMemory, afterRebuildSelectionMemory),
				indexedSelectionMemory: memoryDelta(
					afterIndexedSelectionMemory,
					afterLayoutMemory,
				),
			},
			resourceMetrics: {
				metrics: metricTotal,
				samples: metricSamples.duplicateIndexMs.length,
				duplicateIndexP75Ms: Number(percentile(metricSamples.duplicateIndexMs, 0.75).toFixed(2)),
				sharedIndexP75Ms: Number(percentile(metricSamples.sharedIndexMs, 0.75).toFixed(2)),
				sharedIndexP75Speedup: Number(
					(percentile(metricSamples.duplicateIndexMs, 0.75) / percentile(metricSamples.sharedIndexMs, 0.75)).toFixed(2),
				),
			},
			largeQueryRetention: {
				queries: LARGE_QUERY_ROOTS.length,
				payloadRowsPerQuery: 25_000,
				retainedPayloadRows,
				retainedPayloadMiB: formatMiB(retainedPayloadBytes),
				retainedQueriesAt89_999Ms: retainedQueriesBeforeExpiry,
				retainedQueriesAt90_000Ms: retainedQueriesAfterExpiry,
			},
		},
		null,
		2,
	),
);
