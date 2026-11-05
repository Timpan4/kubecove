import type { Page } from "playwright-core";

export async function checkLogCoverage(page: Page, check: string): Promise<void> {
	if (check === "emit-log") {
		await page.evaluate("import('/src/lib/tauri-dev-mock-coverage.ts').then(m => m.emitCoverageLog())");
	} else if (check === "log-wrap") {
		const row = page.locator("[data-log-row]").first();
		const before = await row.boundingBox();
		if (!before) throw new Error("Missing long log row");
		await page.getByRole("button", { name: "Wrap lines", exact: true }).click();
		await page.waitForFunction((height) => (document.querySelector("[data-log-row]")?.getBoundingClientRect().height ?? height) < height, before.height);
		await page.getByRole("button", { name: "Wrap lines", exact: true }).click();
		await page.waitForFunction((height) => document.querySelector("[data-log-row]")?.getBoundingClientRect().height === height, before.height);
	} else if (check === "log-scroll-back") {
		const viewport = page.locator("[data-log-row]").first().locator('xpath=ancestor::div[contains(@class,"overflow-auto")][1]');
		const height = await viewport.evaluate((element) => element.scrollHeight);
		await viewport.hover();
		await page.mouse.wheel(0, -height);
		await page.waitForFunction((element) => element?.scrollTop === 0, await viewport.elementHandle());
	} else if (check === "log-jump") {
		await page.getByRole("button", { name: /1 new line/ }).click();
		await page.getByText("Newest controlled line", { exact: true }).waitFor();
		await page.getByRole("button", { name: /1 new line/ }).waitFor({ state: "hidden" });
	} else throw new Error(`Unknown log check ${check}`);
}
