import assert from "node:assert/strict";
import { QueryClient } from "@tanstack/query-core";
import { describe } from "vitest";
import {
	buildArgoSearchResources,
	buildDedupedResourceSearchIndex,
	buildGlobalSearchFetchKeys,
	buildPaletteResourceSearchIndex,
	filterNamespaces,
	buildNamespaceSearchIndex,
	NAMESPACE_RESULT_CAP,
	RESOURCE_RESULT_CAP,
} from "@/features/command-palette/entries";
import { filterResourceSearchIndex } from "@/features/resources/helpers";
import { kindKey } from "@/lib/kind-identity";
import type { ResourceScopeResult } from "@/lib/resource-scope-types";
import type {
	ArgoApplicationSetSummary,
	ArgoApplicationSummary,
	ArgoAppProjectSummary,
	DiscoveredResourceKind,
	ResourceSummary,
} from "@/lib/types";
import { benchmark } from "./benchmark";

function resource(index: number): ResourceSummary {
	return {
		cluster: "prod/context-0",
		apiVersion: index % 2 === 0 ? "v1" : "apps/v1",
		kind: index % 2 === 0 ? "Pod" : "Deployment",
		name: `resource-${index}`,
		namespace: `namespace-${index % 100}`,
		age: "1m",
		health: "healthy",
	};
}

const allRows = Array.from({ length: 10_000 }, (_, index) => resource(index));
const warmedRows = allRows.slice(0, 5_000);
const cachedRowsA = allRows.slice(2_500, 7_500);
const cachedRowsB = allRows.slice(5_000, 10_000);
const resourceSets = [warmedRows, cachedRowsA];
const namespaces = Array.from(
	{ length: 1_000 },
	(_, index) => `namespace-${index}`,
);
const customResourceKinds: DiscoveredResourceKind[] = Array.from(
	{ length: 100 },
	(_, index) => ({
		group: `operator-${index % 40}.example.com`,
		version: "v1",
		apiVersion: `operator-${index % 40}.example.com/v1`,
		kind: `Widget${index}`,
		plural: `widgets${index}`,
		namespaced: index !== 99,
	}),
);
const fluxResourceKinds = customResourceKinds.slice(0, 40);
const firstFluxResourceKind = fluxResourceKinds[0];
assert(firstFluxResourceKind);
const providerAndWorkspaceKeys = buildGlobalSearchFetchKeys(
	namespaces.slice(0, 100),
	customResourceKinds,
	fluxResourceKinds,
);
const argoApps: ArgoApplicationSummary[] = Array.from(
	{ length: 2_000 },
	(_, index) => ({
		name: `application-${index}`,
		cluster: "prod/context-0",
		namespace: "argocd",
		project: `project-${index % 250}`,
		syncStatus: index % 5 === 0 ? "OutOfSync" : "Synced",
		healthStatus: index % 11 === 0 ? "Degraded" : "Healthy",
		destinationNamespace: `namespace-${index % 100}`,
		destinationServer: "https://kubernetes.default.svc",
		sourceRepo: "https://example.com/platform.git",
		sourceRevision: `revision-${index}`,
		resourceNamespaces: [`namespace-${index % 100}`],
		age: "1m",
	}),
);
const argoAppSets: ArgoApplicationSetSummary[] = Array.from(
	{ length: 250 },
	(_, index) => ({
		name: `application-set-${index}`,
		cluster: "prod/context-0",
		namespace: "argocd",
		age: "1m",
		project: `project-${index}`,
		status: "Healthy",
		syncStatus: "Synced",
		healthStatus: "Healthy",
		destinationNamespace: `namespace-${index % 100}`,
		destinationServer: "https://kubernetes.default.svc",
		sourceRepo: "https://example.com/platform.git",
		sourceRevision: "main",
	}),
);
const argoProjects: ArgoAppProjectSummary[] = Array.from(
	{ length: 250 },
	(_, index) => ({
		name: `project-${index}`,
		cluster: "prod/context-0",
		namespace: "argocd",
		age: "1m",
		description: `Project ${index}`,
		status: "Active",
	}),
);
const argoRows = buildArgoSearchResources(argoApps, argoAppSets, argoProjects);
const queryClient = new QueryClient();
queryClient.setQueryData(
	["resources", "source-0", "prod/context-0", "cache-a"],
	{ resources: cachedRowsA, completedRequests: [], failures: [] },
);
queryClient.setQueryData(
	["resources", "source-0", "prod/context-0", "cache-b"],
	{ resources: cachedRowsB, completedRequests: [], failures: [] },
);
const cachedEntries = queryClient.getQueriesData<ResourceScopeResult>({
	queryKey: ["resources", "source-0", "prod/context-0"],
});
const paletteIndex = buildPaletteResourceSearchIndex(
	warmedRows,
	argoRows,
	cachedEntries,
);
const namespaceIndex = buildNamespaceSearchIndex(namespaces);
const term = "resource-";
const namespaceTerm = "namespace-";
const unboundedResourceMatches = filterResourceSearchIndex(paletteIndex, term, "");
const unboundedNamespaceMatches = namespaces.filter((name) => name.toLowerCase().includes(namespaceTerm));
const typedResourceResults = visibleResourceResults(term);
const typedNamespaceResults = visibleNamespaceResults(namespaceTerm);

function visibleResourceResults(query: string): ResourceSummary[] {
	return query.trim()
		? filterResourceSearchIndex(paletteIndex, query, "", RESOURCE_RESULT_CAP)
		: [];
}

function visibleNamespaceResults(query: string): string[] {
	return query.trim() ? filterNamespaces(namespaceIndex, query) : [];
}

const firstCacheIndex = buildDedupedResourceSearchIndex(resourceSets);

assert.equal(cachedEntries.length, 2);
assert.equal(argoRows.length, 2_500);
assert.equal(firstCacheIndex.length, 7_500);
assert.equal(paletteIndex.length, 12_500);
assert(unboundedResourceMatches.length > RESOURCE_RESULT_CAP);
assert(unboundedNamespaceMatches.length > NAMESPACE_RESULT_CAP);
assert.deepEqual(typedResourceResults, unboundedResourceMatches.slice(0, RESOURCE_RESULT_CAP));
assert.deepEqual(typedNamespaceResults, unboundedNamespaceMatches.slice(0, NAMESPACE_RESULT_CAP));
assert.deepEqual(visibleResourceResults(term.toUpperCase()), typedResourceResults);
assert.deepEqual(visibleNamespaceResults(namespaceTerm.toUpperCase()), typedNamespaceResults);
assert.equal(visibleResourceResults("no-such-resource").length, 0);
assert.equal(visibleNamespaceResults("no-such-namespace").length, 0);
assert.deepEqual(visibleResourceResults(""), []);
assert.deepEqual(visibleResourceResults(" \t "), []);
assert.deepEqual(visibleNamespaceResults(""), []);
assert.deepEqual(visibleNamespaceResults(" \t "), []);
assert(
	providerAndWorkspaceKeys.some(
		(key) =>
			kindKey(key.kind) ===
				kindKey(firstFluxResourceKind) &&
			key.namespace === undefined,
	),
);
const clusterScopedCustomKind = customResourceKinds[99];
const namespacedCustomKind = customResourceKinds[40];
assert(namespacedCustomKind);
for (const namespace of namespaces.slice(0, 100)) {
	assert(providerAndWorkspaceKeys.some((key) => key.kind === "Pod" && key.namespace === namespace));
	assert(providerAndWorkspaceKeys.some((key) =>
		kindKey(key.kind) === kindKey(namespacedCustomKind) &&
		key.namespace === namespace));
}
assert(clusterScopedCustomKind);
assert(
	providerAndWorkspaceKeys.some(
		(key) =>
			kindKey(key.kind) ===
				kindKey(clusterScopedCustomKind) &&
			key.namespace === undefined,
	),
);

describe("F36 palette cache harvest, F37 retained typing, and folded F38 search scope", () => {
	benchmark(
		"F36 query cache harvest and cold index with warmed, Argo, and cached rows",
		() => {
			const cached = queryClient.getQueriesData<ResourceScopeResult>({
				queryKey: ["resources", "source-0", "prod/context-0"],
			});
			buildPaletteResourceSearchIndex(warmedRows, argoRows, cached);
		},
	);

	benchmark(
		"F36 one-pass dedupe and index across the existing overlapping resource sets",
		() => {
			buildDedupedResourceSearchIndex(resourceSets);
		},
	);

	benchmark(
		"F37 retained resource and namespace typing with production caps",
		() => {
			visibleResourceResults(term);
			visibleNamespaceResults(namespaceTerm);
			visibleResourceResults("RESOURCE-99");
			visibleNamespaceResults("NAMESPACE-99");
			visibleResourceResults("no-such-resource");
			visibleNamespaceResults("no-such-namespace");
			visibleResourceResults(" \t ");
			visibleNamespaceResults(" \t ");
		},
	);

	benchmark("F37 broad query with uncapped scans as a comparison", () => {
		filterResourceSearchIndex(paletteIndex, term, "").slice(0, RESOURCE_RESULT_CAP);
		namespaces.filter((name) => name.toLowerCase().includes(namespaceTerm)).slice(0, NAMESPACE_RESULT_CAP);
	});

	benchmark(
		"F38 provider-wide Flux keys beside workspace-scoped Kubernetes keys",
		() => {
			buildGlobalSearchFetchKeys(
				namespaces.slice(0, 100),
				customResourceKinds,
				fluxResourceKinds,
			);
		},
	);
});
