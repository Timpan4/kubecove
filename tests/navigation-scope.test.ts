import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { buildFetchKeys } from "../src/features/resources/helpers";
import { canQueryResourceScope } from "../src/app/viewHelpers";
import { CLUSTER_SCOPED_KINDS } from "../src/lib/types";
import { resolveTreeScope } from "../src/lib/tree-nav";

describe("navigation scope", () => {
	test("keeps launcher settings and update controls in app top bar", () => {
		const source = readFileSync("src/app/LauncherShell.tsx", "utf8");
		const topBar = source.indexOf("<AppTopBar");
		const launcher = source.indexOf("<WorkspaceLauncher");

		expect(topBar).toBeGreaterThanOrEqual(0);
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

	test("overview Resources shortcut opens the saved workspace scope directly", () => {
		const source = readFileSync("src/app/useAppNavigation.ts", "utf8");
		const handlerStart = source.indexOf("const handleOpenResources = (");
		const handlerEnd = source.indexOf("const handleOpenArgo =", handlerStart);
		const handlerSource = source.slice(handlerStart, handlerEnd);

		expect(handlerSource).toContain("setSelectedKinds(workspace.scope.kinds)");
		expect(handlerSource).toContain('openView("resources"');
		expect(handlerSource).toContain("treeNode: null");
		expect(handlerSource.indexOf("setSelectedKinds(workspace.scope.kinds)")).toBeLessThan(
			handlerSource.indexOf('openView("resources"'),
		);
	});

	test("workspace card keyboard order keeps Open before edit and delete", () => {
		const source = readFileSync(
			"src/features/workspaces/WorkspaceLauncher.tsx",
			"utf8",
		);
		const actionStart = source.indexOf("aria-label={`${workspace.name} actions`}");
		const actionEnd = source.indexOf("</CardAction>", actionStart);
		const actions = source.slice(actionStart, actionEnd);

		expect(actions.indexOf("Open ${workspace.name}")).toBeGreaterThanOrEqual(0);
		expect(actions.indexOf("Open ${workspace.name}")).toBeLessThan(
			actions.indexOf("Edit ${workspace.name}"),
		);
		expect(actions.indexOf("Edit ${workspace.name}")).toBeLessThan(
			actions.indexOf("Delete ${workspace.name}"),
		);
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
