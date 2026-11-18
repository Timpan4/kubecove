import type { WebActions } from "./web-action";
import type { Page } from "playwright-core";

const pastedPaths = (page: Page) => page.getByRole("textbox", { name: "Paste kubeconfig paths", exact: true });

/** Kubeconfig source actions in the settings page. Remote mode uses the server path field. */
export const kubeconfigSettingsActions = {
	async addServerKubeconfigPath(page, step) {
		await page.getByRole("textbox", { name: "Kubeconfig path on server", exact: true }).fill(step.value);
		await page.getByRole("button", { name: "Add server path", exact: true }).click();
		return;
	},
	async addPastedKubeconfigPaths(page, step) {
		await pastedPaths(page).fill(step.value);
		await page.getByRole("button", { name: "Add pasted paths", exact: true }).click();
		return;
	},
	async fillKubeconfigPathDraft(page, step) {
		await pastedPaths(page).fill(step.value);
		return;
	},
	async expectKubeconfigPathDraft(page, step) {
		// The field of the current mode: the server path when remote, else the pasted paths.
		const field = page.getByRole("textbox", { name: "Kubeconfig path on server", exact: true }).or(pastedPaths(page));
		const handle = await field.elementHandle();
		await page.waitForFunction(({ field, value, enabled }) =>
			(field instanceof HTMLInputElement || field instanceof HTMLTextAreaElement) && field.value === value && field.disabled !== enabled,
		{ field: handle, value: step.value, enabled: step.enabled }).catch((error) => {
			throw new Error(`Kubeconfig path draft is not ${JSON.stringify(step.value)} with enabled ${step.enabled}: ${String(error)}`);
		});
		return;
	},
	async expectAccessibleName(page, step) {
		await page.getByLabel(step.name, { exact: true }).waitFor();
		return;
	},
	async clickKubeconfigPathButton(page, step) {
		// Row buttons share their labels, so the row is the innermost container that holds both the path and the button.
		const button = page.locator("div")
			.filter({ has: page.getByText(step.value, { exact: true }) })
			.filter({ has: page.getByRole("button", { name: step.label, exact: true }) })
			.last()
			.getByRole("button", { name: step.label, exact: true });
		await button.click();
		return;
	},
	async expectKubeconfigPathBefore(page, step) {
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
	},
} satisfies Partial<WebActions>;
