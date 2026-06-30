import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { buildFetchKeys } from "../src/features/resources/helpers";
import { CLUSTER_SCOPED_KINDS } from "../src/lib/types";
import { resolveTreeScope } from "../src/lib/tree-nav";

describe("navigation scope", () => {
	test("keeps launcher settings and update controls in app top bar", () => {
		const source = readFileSync("src/app/svelte/App.svelte", "utf8");
		const topBar = source.indexOf("<header");
		const launcher = source.indexOf("<WorkspaceLauncher");

		expect(topBar).toBeGreaterThanOrEqual(0);
		expect(launcher).toBeGreaterThan(topBar);
		expect(source).toContain("UpdateStatusButton");
		expect(source).toContain('aria-label="Open settings"');
		expect(source).not.toContain("RuntimeBadge");
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

	test("overview Resources shortcut opens the saved workspace scope directly", () => {
		const source = readFileSync("src/app/svelte/WorkspaceShell.svelte", "utf8");
		const handlerStart = source.indexOf("function openResources(");
		const handlerEnd = source.indexOf("function openArgo(", handlerStart);
		const handlerSource = source.slice(handlerStart, handlerEnd);

		expect(handlerSource).toContain("selectedNode =");
		expect(handlerSource).toContain('viewMode = "resources"');
		expect(handlerSource).toContain("resourceNamespaceOverride");
	});

	test("workspace card keyboard order keeps Open before edit and delete", () => {
		const source = readFileSync(
			"src/features/workspaces/WorkspaceLauncher.svelte",
			"utf8",
		);
		const workspaceNameToken = "$" + "{workspace.name}";
		const actionStart = source.indexOf(`aria-label={\`${workspaceNameToken} actions\`}`);
		const actionEnd = source.indexOf("</CardAction>", actionStart);
		const actions = source.slice(actionStart, actionEnd);

		expect(actions.indexOf(`Open ${workspaceNameToken}`)).toBeGreaterThanOrEqual(0);
		expect(actions.indexOf(`Open ${workspaceNameToken}`)).toBeLessThan(
			actions.indexOf(`Edit ${workspaceNameToken}`),
		);
		expect(actions.indexOf(`Edit ${workspaceNameToken}`)).toBeLessThan(
			actions.indexOf(`Delete ${workspaceNameToken}`),
		);
	});

	test("port forwards section resolves to the workspace management view", () => {
		const source = readFileSync("src/app/svelte/WorkspaceShell.svelte", "utf8");
		const handlerStart = source.indexOf("function openPortForwards()");
		const handlerEnd = source.indexOf("function setAutoStartSavedPortForwards", handlerStart);
		const handlerSource = source.slice(handlerStart, handlerEnd);
		const scope = resolveTreeScope({
			type: "section",
			section: "portForwards",
		});

		expect(scope.portForwardMode).toBe(true);
		expect(scope.kinds).toEqual([]);
		expect(handlerSource).toContain('section: "portForwards"');
		expect(handlerSource).toContain('viewMode = "portForwards"');
	});
});
