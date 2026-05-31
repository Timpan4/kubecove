import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { buildFetchKeys } from "../src/features/resources/helpers";
import { canQueryResourceScope } from "../src/app/viewHelpers";
import { CLUSTER_SCOPED_KINDS } from "../src/lib/types";
import { resolveTreeScope } from "../src/lib/tree-nav";

describe("navigation scope", () => {
	test("keeps launcher settings and update controls in app top bar", () => {
		const source = readFileSync("src/App.tsx", "utf8");
		const launcherGate = source.indexOf("if (!activeWorkspace)");
		const topBar = source.indexOf("<AppTopBar", launcherGate);
		const launcher = source.indexOf("<WorkspaceLauncher", launcherGate);

		expect(launcherGate).toBeGreaterThanOrEqual(0);
		expect(topBar).toBeGreaterThan(launcherGate);
		expect(launcher).toBeGreaterThan(topBar);
		expect(source).toContain("showClusterSelector={false}");
		expect(source).toContain("showSearch={false}");
	});

	test("selecting Namespaces scopes to all namespaced resource kinds", () => {
		const scope = resolveTreeScope({ type: "section", section: "namespaces" });
		const clusterScopedKinds = new Set<string>(CLUSTER_SCOPED_KINDS);

		expect(scope.section).toBe("namespaces");
		expect(scope.namespace).toBeNull();
		expect(scope.kinds.length).toBeGreaterThan(0);
		expect(scope.kinds.every((kind) => !clusterScopedKinds.has(String(kind)))).toBe(
			true,
		);
	});

	test("empty namespace selection fetches all namespaces for typed kinds", () => {
		expect(buildFetchKeys([], ["Pod", "Service"])).toEqual([
			{ kind: "Pod", namespace: undefined },
			{ kind: "Service", namespace: undefined },
		]);
	});

	test("workspace fallback queries only when no tree section is selected", () => {
		expect(
			canQueryResourceScope({
				clusterContext: "admin@solid-k8s",
				kinds: ["Pod"],
				namespaces: [],
				scope: resolveTreeScope(null),
				hasActiveWorkspace: true,
			}),
		).toBe(true);

		expect(
			canQueryResourceScope({
				clusterContext: "admin@solid-k8s",
				kinds: ["Pod"],
				namespaces: [],
				scope: resolveTreeScope({ type: "section", section: "discovered" }),
				hasActiveWorkspace: true,
			}),
		).toBe(false);
	});

	test("port forwards section resolves to the workspace management view", () => {
		const scope = resolveTreeScope({
			type: "section",
			section: "portForwards",
		});

		expect(scope.portForwardMode).toBe(true);
		expect(scope.kinds).toEqual([]);
		expect(
			canQueryResourceScope({
				clusterContext: "admin@solid-k8s",
				kinds: scope.kinds,
				namespaces: [],
				scope,
				hasActiveWorkspace: true,
			}),
		).toBe(false);
	});
});
