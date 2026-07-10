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
import type {
	DiscoveredResourceKind,
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
		},
		null,
		2,
	),
);
