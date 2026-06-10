import { describe, expect, test } from "bun:test";
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

	test("argo entries are hidden when argo is not detected", () => {
		const ids = buildNavigationEntries(false).map((entry) => entry.id);
		expect(ids).not.toContain("section:argo");
		expect(ids).not.toContain("kind:argo:Applications");
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
