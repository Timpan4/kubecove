import type { Page } from "playwright-core";
import type { Step } from "./contract";

export async function detailStatusAction(page: Page, step: Step) {
	switch (step.action) {
		case "toggleDetailDisclosure":
			await page.getByText(step.label, { exact: true }).first().click();
			break;
		// Closed `<details>` keep their children in the DOM, so hidden is the web equivalent of absent.
		case "expectNoPageText":
			await page.getByText(step.text, { exact: true }).first().waitFor({ state: "hidden" });
			break;
		default:
			throw new Error(`unhandled detail status action: ${step.action}`);
	}
}
