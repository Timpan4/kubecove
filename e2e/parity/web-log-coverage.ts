import type { Page } from "playwright-core";

function clipsHorizontalOverflow(element: Element): boolean {
	return element.scrollWidth > element.clientWidth &&
		["clip", "hidden"].includes(getComputedStyle(element).overflowX);
}

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
		const headline = row.locator("[data-log-headline]");
		const fullHeadline = await headline.textContent();
		const short = page.locator("[data-log-row]").nth(1).locator("[data-log-summary]");
		await short.waitFor();
		const [collapsed, control] = await Promise.all([summary.boundingBox(), short.boundingBox()]);
		const clipsOverflow = await summary.evaluate(clipsHorizontalOverflow);
		const viewport = page.locator("[data-log-viewport]");
		const viewportOverflows = await viewport.evaluate((element) => element.scrollWidth > element.clientWidth);
		if (!collapsed || !control || collapsed.height !== control.height) throw new Error("Collapsed log summary spans more than one line");
		if (!clipsOverflow) throw new Error("Long log text should overflow and clip inside its collapsed summary");
		if (viewportOverflows) throw new Error("Collapsed log text creates horizontal scrolling");
		await row.click();
		await row.getByRole("button", { name: "Copy line", exact: true }).waitFor();
		if (await headline.textContent() !== fullHeadline) throw new Error("Expanded log row lost part of the headline");
		const expanded = await summary.boundingBox();
		if (!expanded || expanded.height <= collapsed.height) throw new Error("Expanded log summary did not wrap");
		if (await summary.evaluate((element) =>
			element.scrollHeight > element.clientHeight || element.scrollWidth > element.clientWidth))
			throw new Error("Expanded log summary still clips its text");
	} else if (check === "log-compact-source") {
		const row = page.locator("[data-log-row]").first();
		const source = row.locator("[data-log-source]");
		const message = row.locator("[data-log-headline]");
		if (await message.textContent() !== "boot payments-api") throw new Error("Missing aggregate log headline");
		if (!await source.evaluate(clipsHorizontalOverflow))
			throw new Error("Long log source should overflow and clip inside its column");
		const summary = row.locator("[data-log-summary]");
		const [summaryBox, sourceBox, headline] = await Promise.all([
			summary.boundingBox(), source.boundingBox(), message.evaluate((element) => {
				const range = document.createRange();
				range.selectNodeContents(element);
				const box = element.getBoundingClientRect();
				const fragments = Array.from(range.getClientRects(), ({ left, right, top, bottom }) => ({ left, right, top, bottom }));
				return { left: box.left, right: box.right, fragments };
			}),
		]);
		const viewport = await page.locator("[data-log-viewport]").boundingBox();
		if (!summaryBox || !sourceBox || !viewport || headline.fragments.length === 0 ||
			headline.fragments.some(({ left, right, top, bottom }) =>
				left < Math.max(headline.left, sourceBox.x + sourceBox.width) ||
				right > Math.min(headline.right, summaryBox.x + summaryBox.width, viewport.x + viewport.width) ||
				top >= sourceBox.y + sourceBox.height || bottom <= sourceBox.y))
			throw new Error("Headline does not fit beside the source on the same line");
		await row.click();
		await row.getByRole("button", { name: "Copy line", exact: true }).waitFor();
		const [expanded, expandedSource] = await Promise.all([summary.boundingBox(), source.boundingBox()]);
		if (!expanded || !expandedSource || expandedSource.y + expandedSource.height > expanded.y ||
			expanded.x !== summaryBox.x || expanded.width !== summaryBox.width)
			throw new Error("Expanded message does not have the full width below the source");
	} else if (check === "log-expanded-field") {
		const row = page.locator("[data-log-row]").first();
		const highlight = row.locator("mark").filter({ hasText: /^status=ready$/ });
		await highlight.waitFor();
		await row.click();
		await row.getByRole("button", { name: "Copy line", exact: true }).waitFor();
		await highlight.waitFor();
	} else if (check === "log-scroll-back") {
		const viewport = page.locator("[data-log-viewport]");
		await page.waitForFunction((element) => {
			if (!element || element.scrollHeight <= element.clientHeight) return false;
			// The same one CSS pixel tolerance used for tail-following in ResourceLogsPane.
			return Math.abs(element.scrollHeight - element.clientHeight - element.scrollTop) <= 1;
		}, await viewport.elementHandle());
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
