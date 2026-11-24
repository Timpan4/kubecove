import { parse, stringify } from "yaml";
import { buildFlowTopologyLayout } from "../src/features/resources/topology-implementation";
import { buildResourceTableModel, buildResourceTableProjection, type ResourceTableState } from "../src/features/resources/resourceTableModel";
import { resourceMetricFor, resourceMetricIndex } from "../src/lib/resource-metrics";
import { resources } from "../src/lib/tauri-dev-mock-data";
import type { ResourceMetricsSummary, ResourceSummary, ResourceTopology } from "../src/lib/types";

function sampleMemory() {
	Bun.gc(true);
	const { rss, heapUsed } = process.memoryUsage();
	return { processRssBytes: rss, jsHeapBytes: heapUsed };
}

function jsonBytes<T>(value: T) {
	return new TextEncoder().encode(JSON.stringify(value)).byteLength;
}

function characterize(label: string, rows: ResourceSummary[]) {
	const source = { rows: rows.length, jsonBytes: jsonBytes(rows) };
	let started = performance.now();
	// One metrics read per context, as the UI makes: the index joins each sample to rows of the context it was read from.
	const metrics = new Map<string, ResourceMetricsSummary>();
	rows.forEach((row, index) => {
		let summary = metrics.get(row.cluster);
		if (!summary) {
			summary = { cluster: row.cluster, availability: { status: "available" }, warnings: [], nodes: [], workloads: [], pods: [] };
			metrics.set(row.cluster, summary);
		}
		summary.pods.push({ kind: row.kind, cluster: row.cluster, name: row.name, namespace: row.namespace, cpuMillicores: index, memoryBytes: index * 1024, sourcePods: [] });
	});
	const metricIndex = resourceMetricIndex([...metrics].map(([clusterContext, summary]) => ({ clusterContext, summary })));
	// Rows keep their identity when no metric applies, as the UI's lookup does.
	let matched = 0;
	const merged = rows.map((row) => {
		const metric = resourceMetricFor(metricIndex, row);
		if (metric) matched += 1;
		return metric && metric !== row.metrics ? { ...row, metrics: metric } : row;
	});
	if (matched !== rows.length) throw new Error(`${label}: ${rows.length - matched} of ${rows.length} rows found no metric sample`);
	const metricsMs = performance.now() - started;
	started = performance.now();
	const projection = buildResourceTableProjection(merged);
	const projectionMs = performance.now() - started;
	const state: ResourceTableState = { search: "", gitOpsFilter: "", healthFilter: "all", sort: { id: "name", desc: false }, pageIndex: 0, collapsedGroups: new Set() };
	const changes: Array<Pick<ResourceTableState, "search" | "sort">> = [
		{ search: "", sort: { id: "name", desc: false } },
		{ search: "pod", sort: { id: "cpu", desc: true } },
		{ search: "", sort: { id: "memory", desc: true } },
		{ search: "no-match", sort: { id: "namespace", desc: false } },
		{ search: "", sort: { id: "name", desc: true } },
		{ search: "", sort: { id: "name", desc: false } },
	];
	const operations = changes.map((change, index) => {
		const before = performance.now();
		const model = buildResourceTableModel(projection, { ...state, ...change });
		return { step: index, durationMs: performance.now() - before, filtered: model.filteredRows.length, display: model.displayRows.length, page: model.pageRows.length, entries: model.entries.length };
	});
	const model = buildResourceTableModel(projection, state);
	const tableMemory = sampleMemory();
	const topology: ResourceTopology = {
		nodes: merged.map((summary, index) => ({ id: `fixture-${index}`, kind: summary.kind, name: summary.name, namespace: summary.namespace, health: "healthy", selectable: true, summary })),
		edges: [], warnings: [],
	};
	started = performance.now();
	let flow: ReturnType<typeof buildFlowTopologyLayout> | null = buildFlowTopologyLayout(topology, null);
	const topologyMs = performance.now() - started;
	const topologyOpenMemory = sampleMemory();
	const flowCounts = { nodes: flow.nodes.length, edges: flow.edges.length };
	flow = null;
	const topologyClosedMemory = sampleMemory();
	started = performance.now();
	const yaml = stringify({ apiVersion: "v1", kind: "Pod", metadata: { name: rows[0]?.name, namespace: rows[0]?.namespace }, spec: { containers: [{ name: "app", image: "fixture" }] } });
	const yamlDocument: unknown = parse(yaml);
	const yamlMs = performance.now() - started;
	const yamlMemory = sampleMemory();
	return {
		label, source,
		derived: { searchIndex: projection.searchIndex.length, mergedRows: merged.length, displayRows: model.displayRows.length, pageRows: model.pageRows.length, tableEntries: model.entries.length, topologyNodes: topology.nodes.length, topologyEdges: topology.edges.length, ...flowCounts },
		payloadEstimates: { metricsJsonBytes: jsonBytes([...metrics.values()]), mergedRowsJsonBytes: jsonBytes(merged), projectionJsonBytes: jsonBytes(projection), topologyJsonBytes: jsonBytes(topology), yamlUtf8Bytes: new TextEncoder().encode(yaml).byteLength, parsedYamlJsonBytes: jsonBytes(yamlDocument) },
		durations: { metricsMs, projectionMs, topologyMs, yamlMs, repeatedSearchSort: operations },
		memory: { tableMemory, topologyOpenMemory, topologyClosedMemory, yamlMemory },
	};
}

export function characterizeMemoryFixtures() {
	const baseline = sampleMemory();
	const normal = characterize("normal-existing-browser-mock", resources.map((row) => ({ ...row })));
	const afterNormalCollection = sampleMemory();
	const large = characterize("large-existing-10000-resource-workload", Array.from({ length: 10_000 }, (_, index) => ({
		...resources[index % resources.length], name: `fixture-pod-${index}`, kind: "Pod", cluster: "fixture", namespace: "profile",
	})));
	return {
		engine: "Bun-JavaScriptCore", payloadEstimate: "utf8-json-bytes-not-heap-size",
		limitations: ["JSON estimates are not additive allocation sizes", "Bun GC is explicit; WebView GC is not", "Imported normal mock data remains reachable throughout", "Process RSS includes allocator high-water marks"],
		baseline, normal, afterNormalCollection, large, afterLargeCollection: sampleMemory(),
	};
}
