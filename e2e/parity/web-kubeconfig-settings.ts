import type { Page } from "playwright-core";
import type { Step } from "./contract";

/** Adds a path on the server through the kubeconfig settings field used in remote mode. */
export async function kubeconfigSettingsAction(page: Page, step: Step): Promise<void> {
	if (step.action !== "addServerKubeconfigPath") throw new Error(`Not a kubeconfig settings action: ${step.action}`);
	await page.getByRole("textbox", { name: "Kubeconfig path on server", exact: true }).fill(step.value);
	await page.getByRole("button", { name: "Add server path", exact: true }).click();
}
