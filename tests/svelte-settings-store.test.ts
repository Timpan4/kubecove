import { afterEach, describe, expect, test } from "bun:test";
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
});
