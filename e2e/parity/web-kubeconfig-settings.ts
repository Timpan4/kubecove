import type { Page } from "playwright-core";
import type { Step } from "./contract";

/** Kubeconfig source actions in the settings page. Remote mode uses the server path field. */
export async function kubeconfigSettingsAction(page: Page, step: Step): Promise<void> {
	switch (step.action) {
		case "addServerKubeconfigPath":
			await page.getByRole("textbox", { name: "Kubeconfig path on server", exact: true }).fill(step.value);
			await page.getByRole("button", { name: "Add server path", exact: true }).click();
			return;
		case "clickKubeconfigPathButton": {
			// Row buttons share their labels, so the row is the innermost container that holds both the path and the button.
			const button = page.locator("div")
				.filter({ has: page.getByText(step.value, { exact: true }) })
				.filter({ has: page.getByRole("button", { name: step.label, exact: true }) })
				.last()
				.getByRole("button", { name: step.label, exact: true });
			await button.click();
			return;
		}
		case "expectKubeconfigPathBefore": {
			const earlierText = page.getByText(step.value, { exact: true }).first();
			const laterText = page.getByText(step.other, { exact: true }).first();
			await Promise.all([earlierText.waitFor(), laterText.waitFor()]);
			const earlier = await earlierText.boundingBox();
			const later = await laterText.boundingBox();
			if (!earlier || !later || earlier.y >= later.y) throw new Error(`${step.value} is not above ${step.other}`);
			return;
		}
		default:
			throw new Error(`Not a kubeconfig settings action: ${step.action}`);
	}
}
