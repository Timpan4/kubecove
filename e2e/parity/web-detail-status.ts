import type { WebActions } from "./web-action";

export const detailStatusActions = {
	async toggleDetailDisclosure(page, step) {
		await page.getByText(step.label, { exact: true }).first().click();
	},
	// Closed `<details>` keep their children in the DOM, so hidden is the web equivalent of absent.
	// Every match must be hidden, as the native driver rejects any visible one.
	async expectNoPageText(page, step) {
		await page.getByText(step.text, { exact: true }).filter({ visible: true }).first().waitFor({ state: "detached" });
	},
} satisfies Partial<WebActions>;
