import type { Page } from "playwright-core";
import type { Step } from "./contract";

/** Kubeconfig source actions in the settings page. Remote mode uses the server path field. */
export async function kubeconfigSettingsAction(page: Page, step: Step): Promise<void> {
	switch (step.action) {
		case "addServerKubeconfigPath":
			await page.getByRole("textbox", { name: "Kubeconfig path on server", exact: true }).fill(step.value);
			await page.getByRole("button", { name: "Add server path", exact: true }).click();
			return;
		case "expectAccessibleName":
			await page.getByRole("textbox", { name: step.name, exact: true }).waitFor();
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
			// Move applies the new order after an asynchronous source update, so wait for the order.
			await page.waitForFunction(({ earlier, later }) => {
				const top = (text: string) => {
					const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
					for (let node = walker.nextNode(); node; node = walker.nextNode()) if (node.textContent?.trim() === text) return node.parentElement?.getBoundingClientRect().top;
					return undefined;
				};
				const [first, second] = [top(earlier), top(later)];
				return first !== undefined && second !== undefined && first < second;
			}, { earlier: step.value, later: step.other }).catch((error) => { throw new Error(`${step.value} is not above ${step.other}: ${String(error)}`); });
			return;
		}
		default:
			throw new Error(`Not a kubeconfig settings action: ${step.action}`);
	}
}
