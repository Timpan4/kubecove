import { afterEach, describe, expect, test } from "bun:test";
import {
	mergePersistedSettings,
	normalizeGitOpsViewMode,
	partializeSettings,
	useSettingsState,
} from "../src/lib/settings";

afterEach(() => {
	useSettingsState.getState().setDebugModeEnabled(false);
	useSettingsState.getState().setShowFullTopologyOnSelection(false);
	useSettingsState.getState().setGitOpsViewMode("cards");
});

describe("settings", () => {
	test("defaults diagnostics mode off and toggles it explicitly", () => {
		expect(useSettingsState.getState().debugModeEnabled).toBe(false);

		useSettingsState.getState().setDebugModeEnabled(true);

		expect(useSettingsState.getState().debugModeEnabled).toBe(true);
	});

	test("focuses topology selections by default and can keep the full map visible", () => {
		expect(useSettingsState.getState().showFullTopologyOnSelection).toBe(false);

		useSettingsState.getState().setShowFullTopologyOnSelection(true);

		expect(useSettingsState.getState().showFullTopologyOnSelection).toBe(true);
	});

	test("defaults GitOps to cards and remembers an explicit list choice", () => {
		expect(useSettingsState.getState().gitOpsViewMode).toBe("cards");

		useSettingsState.getState().setGitOpsViewMode("list");

		expect(useSettingsState.getState().gitOpsViewMode).toBe("list");
	});

	test("falls back to cards for invalid persisted GitOps view modes", () => {
		expect(normalizeGitOpsViewMode("list")).toBe("list");
		expect(normalizeGitOpsViewMode("cards")).toBe("cards");
		expect(normalizeGitOpsViewMode("grid")).toBe("cards");
		expect(normalizeGitOpsViewMode(undefined)).toBe("cards");
	});

	test("persists and validates GitOps view mode through the configured settings storage", () => {
		const current = useSettingsState.getState();

		expect(partializeSettings({ ...current, gitOpsViewMode: "list" })).toMatchObject({
			gitOpsViewMode: "list",
		});
		expect(mergePersistedSettings({ gitOpsViewMode: "list" }, current)).toMatchObject({
			gitOpsViewMode: "list",
		});
		expect(mergePersistedSettings({ gitOpsViewMode: "grid" }, current)).toMatchObject({
			gitOpsViewMode: "cards",
		});
	});
});
