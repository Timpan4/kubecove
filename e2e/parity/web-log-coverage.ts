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
		const summary = row.locator("[data-log-summary]");
		const collapsed = await summary.evaluate((element) => ({
			height: element.clientHeight,
			lineHeight: Number.parseFloat(getComputedStyle(element).lineHeight),
			overflows: element.scrollWidth > element.clientWidth,
		}));
		const viewport = page.locator("[data-log-viewport]");
		const viewportOverflows = await viewport.evaluate((element) => element.scrollWidth > element.clientWidth);
		if (collapsed.height !== collapsed.lineHeight) throw new Error("Collapsed log summary spans more than one line");
		if (!collapsed.overflows) throw new Error("Long log fixture fits without clipping");
		if (viewportOverflows) throw new Error("Collapsed log text creates horizontal scrolling");
		await row.click();
		await row.getByRole("button", { name: "Copy line", exact: true }).waitFor();
		const expanded = await summary.boundingBox();
		if (!expanded || expanded.height <= collapsed.height) throw new Error("Expanded log summary did not wrap");
	} else if (check === "log-compact-source") {
		const row = page.locator("[data-log-row]").first();
		const source = row.locator("[data-log-source]");
		const message = row.getByText("boot payments-api", { exact: true });
		if (!await source.evaluate((element) => element.scrollWidth > element.clientWidth))
			throw new Error("Long source fixture should be wider than its column");
		const [summary, headline] = await Promise.all([row.locator("[data-log-summary]").boundingBox(), message.boundingBox()]);
		if (!summary || !headline || headline.x + headline.width > summary.x + summary.width)
			throw new Error("Source leaves too little room for the headline");
	} else if (check === "log-scroll-back") {
		const viewport = page.locator("[data-log-viewport]");
		const height = await viewport.evaluate((element) => element.scrollHeight);
		await viewport.hover();
		await page.mouse.wheel(0, -height);
		await page.waitForFunction((element) => element?.scrollTop === 0, await viewport.elementHandle());
	} else if (check === "log-jump") {
		await page.getByRole("button", { name: /1 new line/ }).click();
		const newest = page.getByText("Newest controlled line", { exact: true });
		await newest.waitFor();
		// The line is in the DOM before the jump, so the jump must scroll it into the log viewport.
		const viewport = page.locator("[data-log-viewport]");
		await page.waitForFunction(([line, box]) => {
			const [row, view] = [line?.getBoundingClientRect(), box?.getBoundingClientRect()];
			return !!row && !!view && row.bottom > view.top && row.top < view.bottom;
		}, [await newest.elementHandle(), await viewport.elementHandle()] as const);
		await page.getByRole("button", { name: /1 new line/ }).waitFor({ state: "hidden" });
	} else throw new Error(`Unknown log check ${check}`);
}
