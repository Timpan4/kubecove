import { afterEach, describe, expect, test } from "bun:test";
import { useSettingsState } from "../src/lib/settings";

afterEach(() => {
	useSettingsState.getState().setDebugModeEnabled(false);
	useSettingsState.getState().setUiRuntimeMode("svelte");
	useSettingsState.getState().setShowFullTopologyOnSelection(false);
});

describe("settings", () => {
	test("defaults diagnostics mode off and toggles it explicitly", () => {
		expect(useSettingsState.getState().debugModeEnabled).toBe(false);

		useSettingsState.getState().setDebugModeEnabled(true);

		expect(useSettingsState.getState().debugModeEnabled).toBe(true);
	});

	test("defaults UI runtime to Svelte and keeps React fallback selectable", () => {
		expect(useSettingsState.getState().uiRuntimeMode).toBe("svelte");

		useSettingsState.getState().setUiRuntimeMode("react");

		expect(useSettingsState.getState().uiRuntimeMode).toBe("react");
	});

	test("focuses topology selections by default and can keep the full map visible", () => {
		expect(useSettingsState.getState().showFullTopologyOnSelection).toBe(false);

		useSettingsState.getState().setShowFullTopologyOnSelection(true);

		expect(useSettingsState.getState().showFullTopologyOnSelection).toBe(true);
	});
});
