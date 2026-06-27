import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import {
	clampDetailPanelSize,
	detailPanelSizeFromKey,
	detailPanelSizeFromPointer,
	DETAIL_PANEL_DEFAULT_SIZE,
	DETAIL_PANEL_MIN_SIZE,
	DETAIL_PANEL_RESIZE_STEP,
	MAIN_PANEL_DEFAULT_SIZE,
	MAIN_PANEL_MIN_SIZE,
} from "../src/app/svelte/detailPanelFrameModel";

describe("svelte detail panel frame", () => {
	test("matches React detail panel sizing defaults", () => {
		expect(MAIN_PANEL_DEFAULT_SIZE).toBe(60);
		expect(DETAIL_PANEL_DEFAULT_SIZE).toBe(40);
		expect(MAIN_PANEL_MIN_SIZE).toBe(30);
		expect(DETAIL_PANEL_MIN_SIZE).toBe(33);
		expect(DETAIL_PANEL_RESIZE_STEP).toBe(2);
	});

	test("clamps detail panel resize to keep both panes usable", () => {
		expect(clampDetailPanelSize(20)).toBe(33);
		expect(clampDetailPanelSize(50)).toBe(50);
		expect(clampDetailPanelSize(90)).toBe(70);
		expect(clampDetailPanelSize(12, 18)).toBe(18);
	});

	test("converts pointer position into right-side detail size", () => {
		expect(detailPanelSizeFromPointer({ width: 1000, right: 1000 }, 600)).toBe(40);
		expect(detailPanelSizeFromPointer({ width: 1000, right: 1000 }, 900)).toBe(33);
		expect(detailPanelSizeFromPointer({ width: 1000, right: 1000 }, 100)).toBe(70);
		expect(detailPanelSizeFromPointer({ width: 0, right: 1000 }, 600)).toBe(40);
		expect(detailPanelSizeFromPointer({ width: 0, right: 1000 }, 600, 22, 18)).toBe(22);
	});

	test("maps keyboard resize keys to the accessible value range", () => {
		expect(detailPanelSizeFromKey(40, "ArrowLeft")).toBe(42);
		expect(detailPanelSizeFromKey(40, "ArrowRight")).toBe(38);
		expect(detailPanelSizeFromKey(34, "ArrowRight")).toBe(33);
		expect(detailPanelSizeFromKey(69, "ArrowLeft")).toBe(70);
		expect(detailPanelSizeFromKey(40, "Home")).toBe(33);
		expect(detailPanelSizeFromKey(40, "End")).toBe(70);
		expect(detailPanelSizeFromKey(40, "Tab")).toBeNull();
		expect(detailPanelSizeFromKey(19, "ArrowRight", 18)).toBe(18);
	});

	test("keeps resize handle accessible", () => {
		const source = readFileSync("src/app/svelte/DetailPanelFrame.svelte", "utf8");

		expect(source).toContain('aria-label="Resize details panel"');
		expect(source).toContain('role="separator"');
		expect(source).toContain('aria-orientation="vertical"');
		expect(source).toContain("onpointerdown={startResize}");
		expect(source).toContain("onkeydown={handleResizeKeydown}");
	});
});
