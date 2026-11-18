import type { WebActions } from "./web-action";
import type { Page } from "playwright-core";

async function expectText(page: Page, selector: string, expected: string) {
	const actual = (await page.getByRole("dialog").locator(selector).textContent())?.trim();
	if (actual !== expected) throw new Error(`${selector} reads ${JSON.stringify(actual)}, not ${JSON.stringify(expected)}`);
}

/** The delete review's target summary, its folded dry-run diff, and absence of a name-entry requirement. */
export const deleteReviewActions = {
	async showReviewDiff(page, step) {
		const dialog = page.getByRole("dialog");
		const disclosure = dialog.getByRole("button", { name: "API server result", exact: true });
		if (step.key) {
			await disclosure.focus();
			await disclosure.press(step.key === "Space" ? " " : "Enter");
		} else await disclosure.click();
		if ((await disclosure.getAttribute("aria-expanded")) !== "true") throw new Error("The API server result did not expand");
	},
	async expectDeleteReview(page, step) {
		const dialog = page.getByRole("dialog");
		const target = dialog.getByRole("region", { name: "Delete target" });
		await target.waitFor();
		const value = async (term: string) => (await target.locator("dt", { hasText: new RegExp(`^${term}$`) }).locator("xpath=following-sibling::dd[1]").textContent())?.trim();
		const name = (await target.locator("p.font-mono").textContent())?.trim();
		if (name !== step.name) throw new Error(`Delete target is ${name}, not ${step.name}`);
		if ((await value("Context")) !== step.context) throw new Error(`Delete context is not ${step.context}`);
		if ((await value("Namespace")) !== step.namespace) throw new Error(`Delete namespace is not ${step.namespace}`);
		const controller = await target.locator("dt", { hasText: /^Controlled by$/ }).count() ? await value("Controlled by") : "";
		if (controller !== step.controller) throw new Error(`Delete controller is ${JSON.stringify(controller)}, not ${JSON.stringify(step.controller)}`);
		await expectText(page, "[data-delete-consequence]", step.consequence);
		const typed = await dialog.getByRole("textbox", { name: "Type resource name to confirm" }).count();
		if (typed > 0) throw new Error("The delete review still asks for the typed name");
	},
} satisfies Partial<WebActions>;
