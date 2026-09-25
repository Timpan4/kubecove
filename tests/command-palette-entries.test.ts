import { describe, expect, test } from "bun:test";
import type { ArgoApplicationSummary, ResourceSummary } from "../src/lib/types";
import {
	buildResourceSearchIndex,
	filterResourceSearchIndex,
} from "../src/features/resources";
import {
	buildArgoSearchResources,
	buildDedupedResourceSearchIndex,
	buildGlobalSearchFetchKeys,
	buildNavigationEntries,
	dedupeResources,
	filterNamespaces,
	filterNavigationEntries,
	resourceEntryKey,
} from "../src/features/command-palette/entries";

function resource(overrides: Partial<ResourceSummary>): ResourceSummary {
	return {
		kind: "Pod",
		cluster: "ctx",
		name: "name",
		namespace: "ns",
		age: "1d",
		...overrides,
	};
}

describe("command palette entries", () => {
	test("GitOps entries are hidden when providers are not detected", () => {
		const ids = buildNavigationEntries(false).map((entry) => entry.id);
		expect(ids).not.toContain("section:argo");
		expect(ids).not.toContain("kind:argo:Applications");
	});

	test("navigation entries keep late actions reachable without query", () => {
		const entries = buildNavigationEntries(true);
		const settingsIndex = entries.findIndex((entry) => entry.id === "action:settings");
		const launcherIndex = entries.findIndex((entry) => entry.id === "action:launcher");

		expect(entries.length).toBeGreaterThan(20);
		expect(settingsIndex).toBeGreaterThanOrEqual(20);
		expect(launcherIndex).toBeGreaterThan(settingsIndex);
		expect(filterNavigationEntries(entries, "")).toHaveLength(entries.length);
	});

	test("search scope covers native and discovered resources across provider namespaces", () => {
		const fluxKind = {
			group: "kustomize.toolkit.fluxcd.io",
			version: "v1",
			apiVersion: "kustomize.toolkit.fluxcd.io/v1",
			kind: "Kustomization",
			plural: "kustomizations",
			namespaced: true,
		};
		const widgetKind = {
			group: "example.com",
			version: "v1",
			apiVersion: "example.com/v1",
			kind: "Widget",
			plural: "widgets",
			namespaced: true,
		};
		const keys = buildGlobalSearchFetchKeys(["team-a"], [widgetKind], [fluxKind]);

		expect(keys).toContainEqual({ kind: "Deployment", namespace: "team-a" });
		expect(keys).toContainEqual({ kind: "Node", namespace: undefined });
		expect(keys).toContainEqual({ kind: widgetKind, namespace: "team-a" });
		expect(
			keys.filter((key) => key.kind === fluxKind),
		).toEqual([{ kind: fluxKind, namespace: undefined }]);
	});

	test("finds a known Argo CD Application by partial name with scope context", () => {
		const application: ArgoApplicationSummary = {
			name: "spotify-weekly-update",
			cluster: "ctx",
			namespace: "argocd",
			project: "default",
			syncStatus: "Synced",
			healthStatus: "Healthy",
			destinationNamespace: "spotify",
			destinationServer: "https://kubernetes.default.svc",
			sourceRepo: "https://github.com/example/spotify.git",
			sourceRevision: "main",
			resourceNamespaces: ["spotify"],
			age: "1d",
		};
		const matches = filterResourceSearchIndex(
			buildDedupedResourceSearchIndex([
				buildArgoSearchResources([application], [], []),
			]),
			"spotify",
			"",
		);

		expect(matches).toHaveLength(1);
		expect(matches[0]).toMatchObject({
			cluster: "ctx",
			kind: "Application",
			name: "spotify-weekly-update",
			namespace: "argocd",
		});
	});

	test("filterNavigationEntries matches case-insensitively", () => {
		const entries = buildNavigationEntries(true);
		const hits = filterNavigationEntries(entries, "WORKLOADS › pod");
		expect(hits.map((entry) => entry.id)).toEqual(["kind:workloads:Pod"]);
		expect(filterNavigationEntries(entries, "")).toHaveLength(entries.length);
	});

	test("filterNamespaces matches substrings", () => {
		expect(filterNamespaces(["argocd", "kube-system", "default"], "rgo")).toEqual([
			"argocd",
		]);
	});

	test("dedupeResources keeps first occurrence by identity key", () => {
		const a = resource({ name: "a" });
		const rows = dedupeResources([a, resource({ name: "a" }), resource({ name: "b" })]);
		expect(rows).toHaveLength(2);
		expect(resourceEntryKey(rows[0])).toBe(resourceEntryKey(a));
	});

	test("deduped resource index matches merged route and keeps warmed rows first", () => {
		const warmed = resource({ name: "checkout", status: "Running" });
		const cachedDuplicate = resource({ name: "checkout", status: "Pending" });
		const cachedOnly = resource({ name: "payments", namespace: "billing" });
		const resourceSets = [[warmed], [cachedDuplicate, cachedOnly]];

		expect(buildDedupedResourceSearchIndex(resourceSets)).toEqual(
			buildResourceSearchIndex(dedupeResources(resourceSets.flat())),
		);
		expect(buildDedupedResourceSearchIndex(resourceSets)[0]?.resource).toBe(warmed);
	});
});
