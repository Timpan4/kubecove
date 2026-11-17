import { sharedActions, type WebActions } from "./web-action";
import type { Page } from "playwright-core";

const escapeRegExp = (value: string) => value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
const healthSummary = (page: Page) => page.locator('[aria-label="Resource health summary"]');
const healthButton = (page: Page, label: string) => healthSummary(page).getByRole("button", { name: new RegExp(`^${escapeRegExp(label)}\\b`) });
const namespaceTrigger = (page: Page) => page.getByRole("button", { name: "Edit namespace scope", exact: true });

export const resourceFilterActions = {
	async openNamespacePicker(page) {
		await namespaceTrigger(page).click();
	},
	async closeNamespacePicker(page) {
		await page.keyboard.press("Escape");
	},
	async searchNamespacePicker(page, step) {
		await page.getByRole("textbox", { name: "Search namespaces", exact: true }).fill(step.value);
	},
	async selectAllNamespaces(page) {
		await page.getByRole("button", { name: "Select all namespaces", exact: true }).click();
	},
	...sharedActions(["toggleNamespace", "expectNamespaceChecked"], async (page, step) => {
		const option = page.locator("label").filter({ has: page.getByText(step.namespace, { exact: true }) }).getByRole("checkbox");
		await option.waitFor();
		const checked = await option.getAttribute("aria-checked") === "true";
		if (step.action === "toggleNamespace") { if (checked !== step.checked) await option.click(); }
		else if (checked !== step.checked) throw new Error(`${step.namespace} checked=${checked}, expected ${step.checked}`);
	}),
	async expectNamespaceTrigger(page, step) {
		await namespaceTrigger(page).getByText(step.label, { exact: true }).waitFor();
	},
	async selectHealthFilter(page, step) {
		await healthButton(page, step.label).click();
	},
	async expectHealthFilter(page, step) {
		await healthButton(page, step.label).and(page.locator(`[aria-pressed="${step.pressed}"]`)).waitFor();
	},
	async chooseGitOpsOwner(page, step) {
		await page.getByRole("combobox").filter({ hasText: /GitOps owners|Owned by/ }).click();
		await page.getByRole("option", { name: step.owner, exact: true }).click();
	},
	async clearResourceFilters(page) {
		await page.getByRole("button", { name: "Clear", exact: true }).click();
	},
	async setHistoricalReplicaSets(page, step) {
		await page.getByRole("checkbox", { name: "Hide historical ReplicaSets", exact: true }).setChecked(step.checked);
	},
} satisfies Partial<WebActions>;
