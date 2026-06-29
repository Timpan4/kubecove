import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import {
	queueUiRuntimeSettingsFocus,
	queueUiRuntimeSettingsOpen,
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
	test("opening Settings consumes only the open request", () => {
		queueUiRuntimeSettingsOpen();
		queueUiRuntimeSettingsFocus();

		expect(takeUiRuntimeSettingsOpen()).toBe(true);
		expect(takeUiRuntimeSettingsOpen()).toBe(false);
		expect(takeUiRuntimeSettingsFocus()).toBe(true);
	});

	test("Svelte app consumes settings queue and path state restore", () => {
		const appSource = readFileSync("src/app/svelte/App.svelte", "utf8");
		const shellSource = readFileSync(
			"src/app/svelte/WorkspaceShell.svelte",
			"utf8",
		);

		expect(appSource).toContain("readPathState()");
		expect(appSource).toContain(
			"workspaceStore.openWorkspace(pathState.workspace.workspaceId)",
		);
		expect(appSource).toContain("takeUiRuntimeSettingsOpen()");
		expect(appSource).toContain("{openSettingsOnWorkspaceMount}");
		expect(appSource).toContain("$settingsStore.debugModeEnabled");
		expect(appSource).toContain("lastDiagnosticsEnabled");
		expect(appSource).not.toContain("settingsStore.subscribe");
		expect(shellSource).toContain("openSettingsOnWorkspaceMount");
		expect(shellSource).toContain("openSettings()");
	});

	test("Svelte badge opens Settings from launcher and workspace chrome", () => {
		const appSource = readFileSync("src/app/svelte/App.svelte", "utf8");
		const shellSource = readFileSync(
			"src/app/svelte/WorkspaceShell.svelte",
			"utf8",
		);

		expect(appSource).toContain("RuntimeBadge");
		expect(appSource).toContain("onOpenSettings={openLauncherSettings}");
		expect(appSource).toContain('launcherView === "settings"');
		expect(appSource).toContain("<SettingsSurface onBack={openWorkspaceLauncher} />");
		expect(shellSource).toContain("RuntimeBadge");
		expect(shellSource).toContain("onOpenSettings={openSettings}");
	});

	test("Svelte usage metric polling keeps previous samples visible", () => {
		const footerSource = readFileSync(
			"src/app/svelte/AppUsageFooter.svelte",
			"utf8",
		);

		expect(footerSource).toContain("placeholderData: (previousData) => previousData");
		expect(footerSource).toContain("queryKeys.appUsageMetrics()");
		expect(footerSource).not.toContain('"svelte-app-usage-metrics"');
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
