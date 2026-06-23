import { afterEach, describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { get } from "svelte/store";
import {
	getSettingsSnapshot,
	settingsStore,
} from "../src/lib/settings-store";
import { useSettingsState } from "../src/lib/settings";

afterEach(() => {
	useSettingsState.getState().setShowUsageFooter(false);
	useSettingsState.getState().setShowUnavailableGitOpsProviders(false);
	useSettingsState.getState().setShowFullTopologyOnSelection(false);
});

describe("svelte settings store", () => {
	test("mirrors shared settings through a Svelte readable store", () => {
		expect(get(settingsStore).showUsageFooter).toBe(false);

		useSettingsState.getState().setShowUsageFooter(true);

		expect(get(settingsStore).showUsageFooter).toBe(true);
		expect(getSettingsSnapshot().showUsageFooter).toBe(true);
	});

	test("exposes the full-topology selection setting to Svelte surfaces", () => {
		expect(get(settingsStore).showFullTopologyOnSelection).toBe(false);

		useSettingsState.getState().setShowFullTopologyOnSelection(true);

		expect(getSettingsSnapshot().showFullTopologyOnSelection).toBe(true);
	});

	test("updates Svelte subscribers when React-readable settings change", () => {
		const values: boolean[] = [];
		const unsubscribe = settingsStore.subscribe((settings) => {
			values.push(settings.showUnavailableGitOpsProviders);
		});

		useSettingsState.getState().setShowUnavailableGitOpsProviders(true);
		unsubscribe();

		expect(values).toEqual([false, true]);
	});

	test("Svelte kubeconfig settings read source state through svelte-query", () => {
		const source = readFileSync(
			"src/app/svelte/KubeconfigSettings.svelte",
			"utf8",
		);

		expect(source).toContain('import { createQuery, useQueryClient } from "@tanstack/svelte-query";');
		expect(source).toContain('queryKey: KUBECONFIG_SOURCES_QUERY_KEY');
		expect(source).toContain("queryFn: () => getKubeconfigSources(client)");
		expect(source).toContain("queryClient.setQueryData(KUBECONFIG_SOURCES_QUERY_KEY, next)");
		expect(source).not.toContain("onMount");
	});

	test("settings surfaces warn that full topology selection can affect performance", () => {
		const svelteSource = readFileSync("src/app/svelte/SettingsSurface.svelte", "utf8");
		const reactSource = readFileSync("src/features/settings/SettingsPage.tsx", "utf8");

		expect(svelteSource).toContain("Keep full map visible during selection");
		expect(svelteSource).toContain("Large namespaces may render slower");
		expect(reactSource).toContain("Keep full map visible during selection");
		expect(reactSource).toContain("Large namespaces may render slower");
	});
});
