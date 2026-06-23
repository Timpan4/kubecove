import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import {
	queueUiRuntimeWorkspaceHandoff,
	queueUiRuntimeSettingsFocus,
	queueUiRuntimeSettingsOpen,
	readPersistedUiRuntimeMode,
	takeUiRuntimeWorkspaceHandoff,
	takeUiRuntimeSettingsFocus,
	takeUiRuntimeSettingsOpen,
} from "../src/lib/ui-runtime";

function makeStorage(): Storage {
	const values = new Map<string, string>();
	return {
		get length() {
			return values.size;
		},
		clear: () => values.clear(),
		getItem: (key) => values.get(key) ?? null,
		key: (index) => Array.from(values.keys())[index] ?? null,
		removeItem: (key) => values.delete(key),
		setItem: (key, value) => values.set(key, value),
	} as Storage;
}

const originalWindow = globalThis.window;

beforeEach(() => {
	Object.defineProperty(globalThis, "window", {
		configurable: true,
		value: {
			localStorage: makeStorage(),
			sessionStorage: makeStorage(),
		},
	});
});

afterEach(() => {
	Object.defineProperty(globalThis, "window", {
		configurable: true,
		value: originalWindow,
	});
});

describe("ui runtime queue helpers", () => {
	test("defaults new installs to React until Svelte cutover gates pass", () => {
		expect(readPersistedUiRuntimeMode()).toBe("react");
	});

	test("keeps persisted React fallback selection", () => {
		window.localStorage.setItem(
			"kubecove-settings",
			JSON.stringify({ state: { uiRuntimeMode: "react" }, version: 0 }),
		);

		expect(readPersistedUiRuntimeMode()).toBe("react");
	});

	test("falls back to React when persisted runtime mode is invalid", () => {
		window.localStorage.setItem(
			"kubecove-settings",
			JSON.stringify({
				state: { uiRuntimeMode: "legacy-react" },
				version: 0,
			}),
		);

		expect(readPersistedUiRuntimeMode()).toBe("react");
	});

	test("falls back to React when persisted runtime settings are corrupt", () => {
		window.localStorage.setItem("kubecove-settings", "{");

		expect(readPersistedUiRuntimeMode()).toBe("react");
	});

	test("opening Settings consumes only the open request", () => {
		queueUiRuntimeSettingsOpen();
		queueUiRuntimeSettingsFocus();

		expect(takeUiRuntimeSettingsOpen()).toBe(true);
		expect(takeUiRuntimeSettingsOpen()).toBe(false);
		expect(takeUiRuntimeSettingsFocus()).toBe(true);
	});

	test("workspace handoff consumes once", () => {
		queueUiRuntimeWorkspaceHandoff("workspace-1");

		expect(takeUiRuntimeWorkspaceHandoff()).toEqual({
			workspaceId: "workspace-1",
		});
		expect(takeUiRuntimeWorkspaceHandoff()).toBeNull();
	});

	test("workspace handoff preserves selected scope payload", () => {
		queueUiRuntimeWorkspaceHandoff({
			workspaceId: "workspace-1",
			selectedNode: {
				type: "kind",
				section: "workloads",
				namespace: "prod",
				kind: "Deployment",
			},
			expandedSections: ["section::workloads"],
			viewMode: "resources",
			resourceInitialSearch: "nginx",
			resourceInitialGitOpsFilter: "checkout",
			resourceInitialHealthFilter: "degraded",
			resourceNamespaceOverride: ["prod"],
		});

		expect(takeUiRuntimeWorkspaceHandoff()).toEqual({
			workspaceId: "workspace-1",
			selectedNode: {
				type: "kind",
				section: "workloads",
				namespace: "prod",
				kind: "Deployment",
			},
			expandedSections: ["section::workloads"],
			viewMode: "resources",
			resourceInitialSearch: "nginx",
			resourceInitialGitOpsFilter: "checkout",
			resourceInitialHealthFilter: "degraded",
			resourceNamespaceOverride: ["prod"],
		});
		expect(takeUiRuntimeWorkspaceHandoff()).toBeNull();
	});

	test("Svelte runtime consumes settings and workspace handoff queues", () => {
		const appSource = readFileSync("src/app/svelte/App.svelte", "utf8");
		const shellSource = readFileSync(
			"src/app/svelte/WorkspaceShell.svelte",
			"utf8",
		);
		const svelteSettingsSource = readFileSync(
			"src/app/svelte/SettingsSurface.svelte",
			"utf8",
		);
		const reactSettingsSource = readFileSync(
			"src/features/settings/SettingsPage.tsx",
			"utf8",
		);

		expect(appSource).toContain("takeUiRuntimeWorkspaceHandoff()");
		expect(appSource).toContain("workspaceStore.openWorkspace(handoff.workspaceId)");
		expect(appSource).toContain("takeUiRuntimeSettingsOpen()");
		expect(appSource).toContain("{openSettingsOnWorkspaceMount}");
		expect(appSource).toContain("{runtimeWorkspaceHandoff}");
		expect(appSource).toContain("$settingsStore.debugModeEnabled");
		expect(appSource).toContain("lastDiagnosticsEnabled");
		expect(appSource).not.toContain("settingsStore.subscribe");
		expect(shellSource).toContain("openSettingsOnWorkspaceMount");
		expect(shellSource).toContain("openSettings()");
		expect(shellSource).toContain("currentWorkspaceHandoff()");
		expect(shellSource).toContain("settingsWorkspaceHandoff");
		expect(svelteSettingsSource).toContain(
			"queueUiRuntimeWorkspaceHandoff(handoff)",
		);
		expect(reactSettingsSource).toContain(
			"queueUiRuntimeWorkspaceHandoff(activeWorkspaceId)",
		);
	});

	test("Svelte runtime badge opens Settings from launcher and workspace chrome", () => {
		const appSource = readFileSync("src/app/svelte/App.svelte", "utf8");
		const shellSource = readFileSync(
			"src/app/svelte/WorkspaceShell.svelte",
			"utf8",
		);

		expect(appSource).toContain("RuntimeBadge");
		expect(appSource).toContain("onOpenSettings={openLauncherSettings}");
		expect(appSource).toContain('launcherView === "settings"');
		expect(appSource).toContain("<SettingsSurface />");
		expect(shellSource).toContain("RuntimeBadge");
		expect(shellSource).toContain("onOpenSettings={openSettings}");
	});

	test("Svelte usage metric polling keeps previous samples visible", () => {
		const footerSource = readFileSync(
			"src/app/svelte/AppUsageFooter.svelte",
			"utf8",
		);
		const settingsSource = readFileSync(
			"src/app/svelte/SettingsSurface.svelte",
			"utf8",
		);

		expect(footerSource).toContain("placeholderData: (previousData) => previousData");
		expect(settingsSource).toContain("placeholderData: (previousData) => previousData");
		expect(footerSource).toContain("queryKeys.appUsageMetrics()");
		expect(settingsSource).toContain("queryKeys.appUsageMetrics()");
		expect(footerSource).not.toContain('"svelte-app-usage-metrics"');
		expect(settingsSource).not.toContain('"svelte-settings-usage-footer"');
	});

	test("Svelte settings rows reserve label room beside controls", () => {
		const rowSource = readFileSync(
			"src/app/svelte/SettingsRow.svelte",
			"utf8",
		);

		expect(rowSource).toContain('orientation="horizontal"');
		expect(rowSource).toContain("flex-1 basis-72");
		expect(rowSource).toContain("sm:gap-4");
	});

	test("Svelte settings search metadata covers nested section rows", () => {
		const settingsSource = readFileSync(
			"src/app/svelte/SettingsSurface.svelte",
			"utf8",
		);

		expect(settingsSource).toContain("const KUBECONFIG_ROWS");
		expect(settingsSource).toContain('title: "Environment variable"');
		expect(settingsSource).toContain('title: "Show source labels"');
		expect(settingsSource).toContain('title: "Added kubeconfig paths"');
		expect(settingsSource).toContain('title: "Latency report"');
		expect(settingsSource).toContain('title: "Topology spike"');
		expect(settingsSource).toContain("const categoryRows");
		expect(settingsSource).toContain("categoryRows[id].some");
	});
});
