import type { Page } from "playwright-core";
import type { Step } from "./contract";

const REFRESH_HINT = "Clear cached data for this view and fetch it again. Filters and selection are preserved.";

/** Opens the live-updates menu, checks what it lists, and closes it again. */
export async function liveMenuAction(page: Page, step: Extract<Step, { action: "expectLiveMenu" }>) {
	await page.getByRole("button", { name: "Live updates and refresh options" }).first().click();
	const menu = page.locator("[data-slot=popover-content]").filter({ has: page.getByText("Current view", { exact: true }) });
	await menu.waitFor();
	await menu.getByText(step.message, { exact: true }).waitFor();
	await menu.getByText(REFRESH_HINT, { exact: true }).waitFor();
	await menu.getByRole("button", { name: "Refresh (clear cache)", exact: true }).waitFor();
	await page.keyboard.press("Escape");
	await menu.waitFor({ state: "hidden" });
}
