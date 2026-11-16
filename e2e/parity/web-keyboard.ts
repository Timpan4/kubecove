import type { Locator, Page } from "playwright-core";

/**
 * Presses Tab until `target` has keyboard focus, as a keyboard user reaches it, so a
 * control outside the Tab order fails the step. Locator `focus()` would also focus an
 * element that Tab skips. One pass of the Tab order visits each tabbable element once,
 * plus the document, so more presses than that means Tab cycled or stayed in a control
 * that keeps Tab, such as the terminal.
 */
export async function tabTo(page: Page, target: Locator): Promise<void> {
	await target.waitFor();
	const focused = () => target.evaluate((node) => node === document.activeElement);
	const tabbable = () => page.evaluate(() => [...document.querySelectorAll<HTMLElement>("*")].filter((node) => node.tabIndex >= 0).length);
	for (let presses = 1; !(await focused()); presses++) {
		const limit = (await tabbable()) + 1;
		if (presses > limit) throw new Error(`Tab pressed ${limit} times, once per tabbable element, without reaching the control`);
		await page.keyboard.press("Tab");
	}
}
