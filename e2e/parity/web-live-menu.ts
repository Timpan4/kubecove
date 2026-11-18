import type { WebActions } from "./web-action";

const REFRESH_HINT = "Clear cached data for this view and fetch it again. Filters and selection are preserved.";

/** Opens the live-updates menu, checks what it lists, and closes it again. */
export const liveMenuActions = {
	async expectLiveMenu(page, step) {
		await page.getByRole("button", { name: "Live updates and refresh options" }).first().click();
		const menu = page.locator("[data-slot=popover-content]").filter({ has: page.getByText("Current view", { exact: true }) });
		await menu.waitFor();
		await menu.getByText(step.message, { exact: true }).waitFor();
		await menu.getByText(REFRESH_HINT, { exact: true }).waitFor();
		await menu.getByRole("button", { name: "Refresh (clear cache)", exact: true }).waitFor();
		await page.keyboard.press("Escape");
		await menu.waitFor({ state: "hidden" });
	},
	async expectLiveState(page, step) {
		await page.getByRole("button", { name: "Live updates and refresh options" }).first().getByText(step.label, { exact: true }).waitFor();
		if (step.notice) await page.getByText(step.notice, { exact: true }).first().waitFor();
	},
	async replayLiveData(page, step) {
		await page.evaluate(`import('/src/lib/tauri-dev-mock-live.ts').then(m => m.replayLiveData(${JSON.stringify(step.script)}))`);
	},
} satisfies Partial<WebActions>;
