import type { Locator, Page } from "playwright-core";

/**
 * Presses Tab until `target` has keyboard focus, as a keyboard user reaches it, so a
 * control outside the Tab order fails the step. Locator `focus()` would also focus an
 * element that Tab skips. Fails when Tab returns to the first element it focused.
 */
export async function tabTo(page: Page, target: Locator): Promise<void> {
	await target.waitFor();
	const focused = () => target.evaluate((node) => node === document.activeElement);
	if (await focused()) return;
	await page.keyboard.press("Tab");
	const first = await page.evaluateHandle(() => document.activeElement);
	while (!(await focused())) {
		await page.keyboard.press("Tab");
		if (await page.evaluate((start) => document.activeElement === start, first)) throw new Error("Tab cycled without reaching the control");
	}
}
