import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import type { ResourceSummary } from "../src/lib/types";
import {
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
	test("navigation entries cover sections, kind children, and actions", () => {
		const entries = buildNavigationEntries(true);
		const ids = entries.map((entry) => entry.id);

		expect(ids).toContain("section:workspaceOverview");
		expect(ids).toContain("section:argo");
		expect(ids).toContain("kind:workloads:Pod");
		expect(ids).toContain("kind:helm:Releases");
		expect(ids).toContain("action:settings");
		expect(ids).toContain("action:launcher");

		const podEntry = entries.find((entry) => entry.id === "kind:workloads:Pod");
		expect(podEntry?.label).toBe("Workloads › Pod");
		expect(podEntry?.nodeId).toEqual({
			type: "kind",
			section: "workloads",
			namespace: undefined,
			group: undefined,
			kind: "Pod",
		});
	});

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

	test("Svelte command palette uses GitOps detection for navigation entries", () => {
		const source = readFileSync("src/app/svelte/CommandPalette.svelte", "utf8");

		expect(source).toContain("detectArgoCD");
		expect(source).toContain("detectFlux");
		expect(source).toContain("buildNavigationEntries(gitOpsNavigationVisible)");
		expect(source).not.toContain("buildNavigationEntries(true)");
		expect(source).not.toContain("filterNavigationEntries(navigationEntries, query).slice(0, 20)");
	});

	test("Svelte command palette uses command selection semantics", () => {
		const palette = readFileSync("src/app/svelte/CommandPalette.svelte", "utf8");
		const dialog = readFileSync("src/components/ui/svelte/CommandDialog.svelte", "utf8");

		expect(palette).toContain("commandProps={{ shouldFilter: false }}");
		expect(palette).toContain("onSelect={() => selectResource(resource)}");
		expect(palette).toContain("onSelect={() => selectNamespace(namespace)}");
		expect(palette).toContain("onSelect={() => selectNavigation(entry)}");
		expect(palette).not.toContain("<CommandItem value={entry.id} onclick=");
		expect(dialog).toContain("commandProps");
		expect(dialog).toContain("{...commandProps}");
	});

	test("Svelte command palette uses shared resource search parity", () => {
		const source = readFileSync("src/app/svelte/CommandPalette.svelte", "utf8");

		expect(source).toContain("const RESOURCE_RESULT_CAP = 50");
		expect(source).toContain("queryKeys.namespaces(workspace.scope.clusterContext, kubeconfigSourceKey)");
		expect(source).toContain("queryKeys.resources(");
		expect(source).toContain('queryKey: ["resources", normalizedSourceKey, workspace.scope.clusterContext]');
		expect(source).toContain("queryClient.getQueriesData<ResourceSummary[]>");
		expect(source).toContain("buildResourceSearchIndex(dedupeResources(merged))");
		expect(source).toContain('filterResourceSearchIndex(resourceSearchIndex, query, "")');
		expect(source).not.toContain('"svelte-command-resources"');
		expect(source).not.toContain('"svelte-command-namespaces"');
		expect(source).not.toContain("function filterResources(");
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
});
