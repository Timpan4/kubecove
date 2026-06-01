import {
	buildFetchKeys,
	buildResourceSearchIndex,
	filterResourceSearchIndex,
	topologyWatchKeys,
	watchKeysFromFetchKeys,
} from "../src/features/resources/helpers";
import type { ResourceSummary } from "../src/lib/types";

const namespaceCount = 100;
const topologyKindCount = 13;
const searchRows = 50_000;
const cacheScopeChanges = 1_000;
const cacheCap = 128;
const rowsPerCachedScope = 5_000;

function forceGc() {
	Bun.gc(true);
}

function memoryBytes() {
	forceGc();
	return process.memoryUsage.rss();
}

function formatMiB(bytes: number) {
	return Number((bytes / 1024 / 1024).toFixed(2));
}

function makeRows(count: number): ResourceSummary[] {
	return Array.from({ length: count }, (_, index) => ({
		cluster: "prod",
		kind: index % 5 === 0 ? "Deployment" : "Pod",
		name: `api-${index}`,
		namespace: `ns-${index % namespaceCount}`,
		age: "1m",
		ownerRef: `owner-${index % 1_000}`,
		argoApp: index % 3 === 0 ? "payments" : "batch",
		helmRelease: index % 7 === 0 ? `rel-${index}` : undefined,
	}));
}

function legacyFilterResources(
	data: ResourceSummary[],
	search: string,
	argoAppFilter: string,
): ResourceSummary[] {
	const term = search.trim().toLowerCase();
	return data.filter((resource) => {
		if (argoAppFilter && resource.argoApp !== argoAppFilter) return false;
		if (!term) return true;
		return [
			resource.name,
			resource.namespace,
			resource.kind,
			resource.apiVersion,
			resource.group,
			resource.plural,
			resource.ownerRef,
			resource.argoApp,
			resource.helmRelease,
		]
			.filter(Boolean)
			.some((value) => String(value).toLowerCase().includes(term));
	});
}

const namespaces = Array.from({ length: namespaceCount }, (_, index) => `ns-${index}`);
const tableKeys = buildFetchKeys(namespaces, ["Pod"]);
const tableWatchKeys = watchKeysFromFetchKeys(tableKeys);
const topologyWatchStreamCount = topologyWatchKeys(namespaces).length;

const rows = makeRows(searchRows);
const baselineMemory = memoryBytes();

let started = performance.now();
let legacyMatches = 0;
for (const query of ["api-4", "owner-99", "payments", "rel-49", "missing"]) {
	legacyMatches += legacyFilterResources(rows, query, "").length;
}
const legacySearchMs = performance.now() - started;
const afterLegacyMemory = memoryBytes();

started = performance.now();
const searchIndex = buildResourceSearchIndex(rows);
const indexBuildMs = performance.now() - started;
const afterIndexMemory = memoryBytes();

started = performance.now();
let indexedMatches = 0;
for (const query of ["api-4", "owner-99", "payments", "rel-49", "missing"]) {
	indexedMatches += filterResourceSearchIndex(searchIndex, query, "").length;
}
const indexedSearchMs = performance.now() - started;
const afterIndexedSearchMemory = memoryBytes();

const sampleScopeBytes =
	JSON.stringify(rows.slice(0, rowsPerCachedScope)).length * 2;
const oldCacheBytes = cacheScopeChanges * sampleScopeBytes;
const cappedCacheBytes = Math.min(cacheScopeChanges, cacheCap) * sampleScopeBytes;

console.log(
	JSON.stringify(
		{
			workload: {
				namespaces: namespaceCount,
				topologyKinds: topologyKindCount,
				searchRows,
				cacheScopeChanges,
				rowsPerCachedScope,
			},
			kubernetesWorkUnits: {
				tablePodNamespaceFetchKeys: tableKeys.length,
				tablePodNamespaceWatchKeys: tableWatchKeys.length,
				topologyWatchesBefore: namespaceCount * topologyKindCount,
				topologyWatchesAfter: topologyWatchStreamCount,
			},
			searchTimingMs: {
				legacy: Number(legacySearchMs.toFixed(2)),
				indexBuild: Number(indexBuildMs.toFixed(2)),
				indexedFiveQueries: Number(indexedSearchMs.toFixed(2)),
			},
			searchMemoryMiB: {
				baselineRows: formatMiB(baselineMemory),
				afterLegacySearch: formatMiB(afterLegacyMemory),
				afterIndexBuild: formatMiB(afterIndexMemory),
				afterIndexedSearch: formatMiB(afterIndexedSearchMemory),
				indexIncremental: formatMiB(afterIndexMemory - afterLegacyMemory),
			},
			cacheRetentionModelMiB: {
				beforeUnbounded: formatMiB(oldCacheBytes),
				afterCapped: formatMiB(cappedCacheBytes),
				reductionPercent: Number(
					(((oldCacheBytes - cappedCacheBytes) / oldCacheBytes) * 100).toFixed(2),
				),
			},
			matches: { legacyMatches, indexedMatches },
		},
		null,
		2,
	),
);
