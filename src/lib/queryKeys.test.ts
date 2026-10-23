import { afterEach, describe, expect, jest, test } from "bun:test";
import { type Query, QueryClient, QueryObserver } from "@tanstack/svelte-query";
import { isFiniteReadQuery } from "./finite-read-lifecycle";
import {
	configureQueryCatalog,
	LARGE_QUERY_GC_TIME_MS,
	queryFamilyRoot,
	queryKeys,
	queryMatchers,
} from "./queryKeys";
import type { ResourceSummary } from "./types";

// Failure modes: a family loses or gains ADR 0009 finite-read metadata, large
// retention drops below its window or replaces finite metadata, a builder emits
// a root the catalog does not declare, and a named matcher selects another
// resource, view, context, or source, or misses a read the ADR requires.

const deployment: ResourceSummary = {
	kind: "Deployment",
	apiVersion: "apps/v1",
	cluster: "dev",
	name: "web",
	namespace: "shop",
	age: "1d",
	health: "healthy",
};
const fluxKind = {
	group: "kustomize.toolkit.fluxcd.io",
	version: "v1",
	apiVersion: "kustomize.toolkit.fluxcd.io/v1",
	kind: "Kustomization",
	plural: "kustomizations",
	namespaced: true,
	category: "kustomize",
};

const finiteKeys = [
	queryKeys.namespaces("dev"),
	queryKeys.resourceKinds("dev"),
	queryKeys.presentCustomResourceKinds("dev", ["shop"]),
	queryKeys.resources("dev", [{ kind: "Pod", namespace: "shop" }]),
	queryKeys.deploymentRevisions(deployment),
	queryKeys.resourceTopology("dev", ["shop"], "ownership"),
	queryKeys.resourceMetrics("dev", ["shop"]),
	queryKeys.resourceDetails(deployment),
	queryKeys.resourceYaml(deployment),
	queryKeys.resourceEvents(deployment, undefined, "uid-1"),
	queryKeys.argoServerDiscovery("dev"),
	queryKeys.argoDetect("dev"),
	queryKeys.argoApps("dev"),
	queryKeys.argoAppSets("dev"),
	queryKeys.argoAppProjects("dev"),
	queryKeys.argoAppDetails("dev", "guestbook", "argocd"),
	queryKeys.argoAppSetDetails("dev", "set", "argocd"),
	queryKeys.argoAppProjectDetails("dev", "default", "argocd"),
	queryKeys.argoWorkspaceInspector("dev", "ws", "guestbook", "argocd", "uid", true, "kubernetes"),
	queryKeys.fluxDetect("dev"),
	queryKeys.fluxResources("dev", fluxKind),
	queryKeys.fluxResourceDetails("dev", fluxKind, "apps", "flux-system"),
	queryKeys.helmReleases("dev"),
	queryKeys.helmReleaseDetails("dev", "shop", "Secret", "release.v1"),
	queryKeys.helmReleaseReconciliation("dev", "shop", "Secret", "release.v1"),
	queryKeys.rbacInspection("dev"),
	queryKeys.incidentCockpit("dev", [{ kind: "Pod", namespace: "shop" }]),
];
const outsideWorkspaceKeys = [
	queryKeys.kubeconfigSources(),
	queryKeys.kubeContexts(),
	queryKeys.argoConnectionStatuses("dev", "ws", ["profile"]),
	queryKeys.portForwards(),
	queryKeys.podExecSessions(),
	queryKeys.appUsageMetrics(),
	queryKeys.backendDiagnostics(),
	queryKeys.backendCacheDiagnostics(),
];
const largeKeys = [
	queryKeys.resources("dev", []),
	queryKeys.resourceTopology("dev", [], "ownership"),
	queryKeys.resourceMetrics("dev", []),
];

function cachedQuery(queryClient: QueryClient, queryKey: readonly unknown[]): Query {
	queryClient.setQueryData(queryKey, "cached");
	const query = queryClient.getQueryCache().find({ queryKey, exact: true });
	if (!query) throw new Error("query was not cached");
	return query;
}

function selected(queryClient: QueryClient, match: (query: Query) => boolean): string[] {
	return queryClient
		.getQueryCache()
		.findAll({ predicate: match })
		.map((query) => query.queryHash)
		.sort();
}

function hashes(queryClient: QueryClient, keys: readonly (readonly unknown[])[]): string[] {
	return keys.map((key) => cachedQuery(queryClient, key).queryHash).sort();
}

afterEach(() => {
	jest.useRealTimers();
});

describe("query catalog classes", () => {
	test("every builder emits a declared family with its finite-read class", () => {
		const queryClient = new QueryClient();
		configureQueryCatalog(queryClient);
		for (const key of finiteKeys) {
			expect(queryFamilyRoot(key)).not.toBe("other");
			expect(isFiniteReadQuery(cachedQuery(queryClient, key))).toBe(true);
		}
		for (const key of outsideWorkspaceKeys) {
			expect(queryFamilyRoot(key)).not.toBe("other");
			expect(isFiniteReadQuery(cachedQuery(queryClient, key))).toBe(false);
		}
		expect(isFiniteReadQuery(cachedQuery(queryClient, queryKeys.idle("closed")))).toBe(false);
		expect(queryFamilyRoot(["unknown-root"])).toBe("other");
		queryClient.clear();
	});

	test("large reads keep finite metadata and expire 300 seconds after their last observer", () => {
		jest.useFakeTimers();
		const queryClient = new QueryClient({ defaultOptions: { queries: { gcTime: 90_000 } } });
		configureQueryCatalog(queryClient);
		const detailsKey = queryKeys.resourceDetails(deployment);
		for (const key of [...largeKeys, detailsKey]) {
			expect(isFiniteReadQuery(cachedQuery(queryClient, key))).toBe(true);
		}
		const reopened = largeKeys[0];

		jest.advanceTimersByTime(90_000);
		expect(queryClient.getQueryData(detailsKey)).toBe(undefined);
		expect(largeKeys.every((key) => queryClient.getQueryData(key) === "cached")).toBe(true);
		const stop = new QueryObserver(queryClient, { queryKey: reopened, enabled: false }).subscribe(() => {});
		jest.advanceTimersByTime(LARGE_QUERY_GC_TIME_MS - 90_000);
		expect(queryClient.getQueryData(largeKeys[1])).toBe(undefined);
		expect(queryClient.getQueryData(reopened)).toBe("cached");
		stop();
		jest.advanceTimersByTime(LARGE_QUERY_GC_TIME_MS - 1);
		expect(queryClient.getQueryData(reopened)).toBe("cached");
		jest.advanceTimersByTime(1);
		expect(queryClient.getQueryData(reopened)).toBe(undefined);
		queryClient.clear();
	});
});

describe("query catalog matchers", () => {
	test("current view selects active finite reads for one context and source", () => {
		const queryClient = new QueryClient();
		const view = [
			queryKeys.resources("dev", []),
			queryKeys.resourceDetails(deployment),
			queryKeys.resourceYaml(deployment),
			queryKeys.resourceEvents(deployment),
			queryKeys.argoApps("dev"),
		];
		const outside = [
			queryKeys.resources("prod", []),
			queryKeys.resources("dev", [], "OTHER"),
			queryKeys.argoConnectionStatuses("dev", "ws", []),
		];
		const stops = [...view, ...outside].map((queryKey) => {
			queryClient.setQueryData(queryKey, "cached");
			return new QueryObserver(queryClient, { queryKey, queryFn: async () => "fresh" }).subscribe(() => {});
		});
		hashes(queryClient, [queryKeys.argoAppSets("dev")]);
		try {
			expect(selected(queryClient, queryMatchers.currentView("dev"))).toEqual(hashes(queryClient, view));
		} finally {
			for (const stop of stops) stop();
			queryClient.clear();
		}
	});

	test("YAML apply refreshes only the applied view of the resource (ADR 0006)", () => {
		const queryClient = new QueryClient();
		const view = { dynamicKindKey: null, yamlViewMode: "applyClean", yamlEncoding: "yaml" } as const;
		const applied = hashes(queryClient, [
			queryKeys.resourceDetails(deployment, undefined, view),
			queryKeys.resourceYaml(deployment, undefined, { ...view, redactSecrets: false }, 3),
		]);
		hashes(queryClient, [
			queryKeys.resourceDetails(deployment),
			queryKeys.resourceYaml(deployment, undefined, { ...view, yamlEncoding: "kyaml" }),
			queryKeys.resourceDetails({ ...deployment, namespace: "other" }, undefined, view),
			queryKeys.resourceDetails(deployment, "OTHER", view),
			queryKeys.resourceEvents(deployment),
			queryKeys.deploymentRevisions(deployment),
		]);

		expect(selected(queryClient, queryMatchers.yamlApply(deployment, undefined, view))).toEqual(applied);
		queryClient.clear();
	});

	test("a resource action refreshes every view, revisions, lists, and topology (ADR 0024)", () => {
		const queryClient = new QueryClient();
		const refreshed = hashes(queryClient, [
			queryKeys.resourceDetails(deployment),
			queryKeys.resourceDetails(deployment, undefined, { yamlViewMode: "applyClean" }),
			queryKeys.resourceYaml(deployment, undefined, { redactSecrets: false }, 2),
			queryKeys.deploymentRevisions(deployment),
			queryKeys.resources("dev", [{ kind: "Deployment", namespace: "shop" }]),
			queryKeys.resourceTopology("dev", ["shop"], "networkFlow"),
		]);
		hashes(queryClient, [
			queryKeys.resourceEvents(deployment),
			queryKeys.resourceMetrics("dev", ["shop"]),
			queryKeys.resourceDetails({ ...deployment, kind: "StatefulSet" }),
			queryKeys.resourceDetails({ ...deployment, apiVersion: "apps/v2" }),
			queryKeys.resourceDetails({ ...deployment, cluster: "prod" }),
			queryKeys.resources("prod", []),
			queryKeys.resources("dev", [], "OTHER"),
		]);

		expect(selected(queryClient, queryMatchers.resourceAction(deployment))).toEqual(refreshed);
		// Details summaries omit apiVersion; Resource identity then matches on the other fields.
		const summary = { ...deployment, apiVersion: undefined };
		const withoutApiVersion = selected(queryClient, queryMatchers.resourceAction(summary));
		expect(withoutApiVersion).toEqual(
			[...refreshed, ...hashes(queryClient, [queryKeys.resourceDetails({ ...deployment, apiVersion: "apps/v2" })])].sort(),
		);
		queryClient.clear();
	});

	test("resource lists match one context and source across fetch scopes", () => {
		const queryClient = new QueryClient();
		const lists = hashes(queryClient, [
			queryKeys.resources("dev", []),
			queryKeys.resources("dev", [{ kind: "Pod", namespace: "shop" }]),
		]);
		hashes(queryClient, [
			queryKeys.resources("prod", []),
			queryKeys.resources("dev", [], "OTHER"),
			queryKeys.incidentCockpit("dev", []),
		]);

		expect(selected(queryClient, queryMatchers.resourceLists("dev"))).toEqual(lists);
		queryClient.clear();
	});

	test("an Argo Application scope covers every UID, transport, and resource variant of only that Application", () => {
		const queryClient = new QueryClient();
		const variants = (cluster = "dev", workspace = "ws", name = "guestbook", namespace = "argocd", source = "KUBECONFIG") => [
			queryKeys.argoWorkspaceApplication(cluster, workspace, name, namespace, "uid-1", true, source),
			queryKeys.argoWorkspaceApplication(cluster, workspace, name, namespace, "uid-2", false, source),
			queryKeys.argoWorkspaceInspector(cluster, workspace, name, namespace, "uid-1", true, "connected", "connection-1", source),
			queryKeys.argoWorkspaceInspector(cluster, workspace, name, namespace, "uid-2", false, "kubernetes", "ignored", source),
			queryKeys.argoWorkspaceSelectedResource(cluster, workspace, name, namespace, "uid-1", true, "connected", "connection-1", "apps", "Deployment", "default", "guestbook", source),
		];
		const scoped = hashes(queryClient, variants());
		for (const other of [
			variants("dev", "ws", "guestbook", "argocd", "SECOND_KUBECONFIG"),
			variants("prod"),
			variants("dev", "ws-2"),
			variants("dev", "ws", "other"),
			variants("dev", "ws", "guestbook", "other"),
		]) hashes(queryClient, other);

		expect(new Set(scoped).size).toBe(variants().length);
		const scope = queryKeys.argoWorkspaceApplicationScope("dev", "ws", "guestbook", "argocd", "KUBECONFIG");
		expect(queryClient.getQueryCache().findAll({ queryKey: scope }).map((query) => query.queryHash).sort()).toEqual(scoped);
		queryClient.clear();
	});

	test("Argo connection status matcher covers every status read and nothing else", () => {
		const queryClient = new QueryClient();
		const statuses = hashes(queryClient, [
			queryKeys.argoConnectionStatuses("dev", "ws", ["a"]),
			queryKeys.argoConnectionStatuses("prod", "ws-2", ["b"], "OTHER"),
		]);
		hashes(queryClient, [queryKeys.argoApps("dev"), queryKeys.argoServerDiscovery("dev")]);

		expect(selected(queryClient, queryMatchers.argoConnectionStatuses())).toEqual(statuses);
		queryClient.clear();
	});
});
