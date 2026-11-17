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
	} else if (check === "log-compact-wrap") {
		const row = page.locator("[data-log-row]").first();
		await row.waitFor();
		const closed = await row.boundingBox();
		// LogEntry uses an 18px summary and 2px of padding on each edge.
		if (closed?.height !== 22) throw new Error("Collapsed log row takes more than one line");
		await row.click();
		await row.getByRole("button", { name: "Copy line", exact: true }).waitFor();
		const summary = await row.locator(":scope > div > div:first-child").boundingBox();
		if (!summary || summary.height <= 18) throw new Error("Expanded log summary did not wrap");
	} else if (check === "log-scroll-back") {
		const viewport = page.locator("[data-log-row]").first().locator('xpath=ancestor::div[contains(@class,"overflow-auto")][1]');
		const height = await viewport.evaluate((element) => element.scrollHeight);
		await viewport.hover();
		await page.mouse.wheel(0, -height);
		await page.waitForFunction((element) => element?.scrollTop === 0, await viewport.elementHandle());
	} else if (check === "log-jump") {
		await page.getByRole("button", { name: /1 new line/ }).click();
		const newest = page.getByText("Newest controlled line", { exact: true });
		await newest.waitFor();
		// The line is in the DOM before the jump, so the jump must scroll it into the log viewport.
		const viewport = newest.locator('xpath=ancestor::div[contains(@class,"overflow-auto")][1]');
		await page.waitForFunction(([line, box]) => {
			const [row, view] = [line?.getBoundingClientRect(), box?.getBoundingClientRect()];
			return !!row && !!view && row.bottom > view.top && row.top < view.bottom;
		}, [await newest.elementHandle(), await viewport.elementHandle()] as const);
		await page.getByRole("button", { name: /1 new line/ }).waitFor({ state: "hidden" });
	} else throw new Error(`Unknown log check ${check}`);
}
