import assert from "node:assert/strict";
import { QueryClient } from "@tanstack/query-core";
import { bench, describe } from "vitest";
import {
	buildArgoSearchResources,
	buildDedupedResourceSearchIndex,
	buildGlobalSearchFetchKeys,
	buildPaletteResourceSearchIndex,
	filterNamespaces,
} from "@/features/command-palette/entries";
import { kindKey } from "@/lib/kind-identity";
import { filterResourceSearchIndex } from "@/features/resources/helpers";
import type { ResourceScopeResult } from "@/lib/resource-scope-types";
import type {
	ArgoApplicationSetSummary,
	ArgoApplicationSummary,
	ArgoAppProjectSummary,
	DiscoveredResourceKind,
	ResourceSummary,
} from "@/lib/types";

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
const typedResourceResults = filterResourceSearchIndex(
	paletteIndex,
	"resource-99",
	"",
).slice(0, 50);
const typedNamespaceResults = filterNamespaces(
	namespaces,
	"namespace-99",
).slice(0, 8);
const unboundedResourceMatches = filterResourceSearchIndex(
	paletteIndex,
	"resource-99",
	"",
);
const unboundedNamespaceMatches = filterNamespaces(namespaces, "namespace-99");
const uppercaseResourceResults = filterResourceSearchIndex(
	paletteIndex,
	"RESOURCE-99",
	"",
).slice(0, 50);
const uppercaseNamespaceResults = filterNamespaces(
	namespaces,
	"NAMESPACE-99",
).slice(0, 8);
const noResourceResults = filterResourceSearchIndex(
	paletteIndex,
	"no-such-resource",
	"",
);
const noNamespaceResults = filterNamespaces(namespaces, "no-such-namespace");

function visibleResourceResults(query: string): ResourceSummary[] {
	return query.trim()
		? filterResourceSearchIndex(paletteIndex, query, "").slice(0, 50)
		: [];
}

function visibleNamespaceResults(query: string): string[] {
	return query.trim() ? filterNamespaces(namespaces, query).slice(0, 8) : [];
}

const firstCacheIndex = buildDedupedResourceSearchIndex(resourceSets);

assert.equal(cachedEntries.length, 2);
assert.equal(argoRows.length, 2_500);
assert.equal(firstCacheIndex.length, 7_500);
assert.equal(paletteIndex.length, 12_500);
assert(typedResourceResults.length <= 50 && typedResourceResults.length > 0);
assert(typedNamespaceResults.length <= 8 && typedNamespaceResults.length > 0);
assert.deepEqual(typedResourceResults, unboundedResourceMatches.slice(0, 50));
assert.deepEqual(typedNamespaceResults, unboundedNamespaceMatches.slice(0, 8));
assert.deepEqual(uppercaseResourceResults, typedResourceResults);
assert.deepEqual(uppercaseNamespaceResults, typedNamespaceResults);
assert.equal(noResourceResults.length, 0);
assert.equal(noNamespaceResults.length, 0);
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
	bench(
		"F36 query cache harvest and cold index with warmed, Argo, and cached rows",
		() => {
			const cached = queryClient.getQueriesData<ResourceScopeResult>({
				queryKey: ["resources", "source-0", "prod/context-0"],
			});
			buildPaletteResourceSearchIndex(warmedRows, argoRows, cached);
		},
	);

	bench(
		"F36 one-pass dedupe and index across the existing overlapping resource sets",
		() => {
			buildDedupedResourceSearchIndex(resourceSets);
		},
	);

	bench(
		"F37 retained resource and namespace typing with production caps",
		() => {
			visibleResourceResults("resource-99");
			visibleNamespaceResults("namespace-99");
			visibleResourceResults("RESOURCE-99");
			visibleNamespaceResults("NAMESPACE-99");
			visibleResourceResults("no-such-resource");
			visibleNamespaceResults("no-such-namespace");
			visibleResourceResults(" \t ");
			visibleNamespaceResults(" \t ");
		},
	);

	bench(
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
