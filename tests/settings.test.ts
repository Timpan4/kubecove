import { afterEach, describe, expect, test } from "bun:test";
import { useSettingsState } from "../src/lib/settings";

afterEach(() => {
	useSettingsState.getState().setDebugModeEnabled(false);
});

describe("settings", () => {
	test("defaults diagnostics mode off and toggles it explicitly", () => {
		expect(useSettingsState.getState().debugModeEnabled).toBe(false);

		useSettingsState.getState().setDebugModeEnabled(true);

		expect(useSettingsState.getState().debugModeEnabled).toBe(true);
	});
});
