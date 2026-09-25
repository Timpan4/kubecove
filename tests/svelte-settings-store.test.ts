import { afterEach, describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { get } from "svelte/store";
import { useSettingsState } from "../src/lib/settings";
import {
	getSettingsSnapshot,
	settingsStore,
} from "../src/lib/settings-store";

afterEach(() => {
	useSettingsState.getState().setShowUsageFooter(false);
	useSettingsState.getState().setShowUnavailableGitOpsProviders(false);
});

describe("svelte settings store", () => {
	test("mirrors shared settings through a Svelte readable store", () => {
		expect(get(settingsStore).showUsageFooter).toBe(false);

		useSettingsState.getState().setShowUsageFooter(true);

		expect(get(settingsStore).showUsageFooter).toBe(true);
		expect(getSettingsSnapshot().showUsageFooter).toBe(true);
	});

	test("updates Svelte subscribers when shared settings change", () => {
		const values: boolean[] = [];
		const unsubscribe = settingsStore.subscribe((settings) => {
			values.push(settings.showUnavailableGitOpsProviders);
		});

		useSettingsState.getState().setShowUnavailableGitOpsProviders(true);
		unsubscribe();

		expect(values).toEqual([false, true]);
	});

	test("opens documentation through an exact Tauri URL permission", () => {
		const settingsSource = readFileSync("src/app/svelte/SettingsSurface.svelte", "utf8");
		const tauriSource = readFileSync("src-tauri/src/lib.rs", "utf8");
		const capability = JSON.parse(
			readFileSync("src-tauri/capabilities/default.json", "utf8"),
		);

		expect(settingsSource).toContain('import { openUrl } from "@tauri-apps/plugin-opener"');
		expect(settingsSource).toContain(
			'const WIKI_URL = "https://github.com/Timpan4/kubecove/wiki"',
		);
		expect(settingsSource).toContain("if (!isTauriRuntime()) return");
		expect(settingsSource).toContain("await openUrl(WIKI_URL)");
		expect(settingsSource).toContain("documentationError = error");
		expect(settingsSource).toContain(
			'fallbackTitle: "KubeCove could not open the documentation"',
		);
		expect(settingsSource).toMatch(
			/<a\s+[\s\S]*?href=\{WIKI_URL\}[\s\S]*?onclick=\{openDocumentation\}[\s\S]*?>/,
		);
		expect(settingsSource).toContain('target="_blank"');
		expect(settingsSource).toContain('rel="noreferrer"');
		expect(tauriSource).toContain(".plugin(tauri_plugin_opener::init())");
		expect(capability.permissions).toContainEqual({
			identifier: "opener:allow-open-url",
			allow: [{ url: "https://github.com/Timpan4/kubecove/wiki" }],
		});
	});
});
