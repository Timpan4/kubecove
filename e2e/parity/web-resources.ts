import { sharedActions, type WebActions } from "./web-action";
import { openPage } from "./web-shell";
import { resourceRow } from "./web-resource-table";
import type { Page } from "playwright-core";
export const resourceActions = {
	async expectResourceStatus(page, step) {
		await resourceRow(page, step).getByText(step.status, { exact: true }).waitFor();
	},
	async openCustomKind(page, step) {
		if (step.surface === "sidebar") {
			await page.getByRole("button", { name: "Expand Custom Resources", exact: true }).click();
			await page.getByRole("button", { name: "Expand kustomize.toolkit.fluxcd.io", exact: true }).click();
			await page.getByRole("treeitem").filter({ has: page.getByText(step.kind, { exact: true }) }).last().click();
		} else {
			await page.getByRole("button", { name: "Edit resource kinds" }).click();
			await page.getByRole("textbox", { name: "Search resource kinds" }).fill(step.kind);
			const checkbox = page.locator("label").filter({ hasText: step.kind }).last().getByRole("checkbox");
			if (await checkbox.getAttribute("aria-checked") !== "true") await checkbox.click();
			await page.keyboard.press("Escape");
		}
	},
	async selectNamespace(page, step) {
		await page.getByRole("treeitem").filter({ has: page.getByText(step.namespace, { exact: true }) }).first().click();
	},
	async openResources(page) {
		await openPage(page, "Workspace Overview");
		await page.getByRole("button", { name: "Open resources", exact: true }).click();
	},
	async search(page, step) {
		await resourceSearch(page).fill(step.value);
	},
	async expectSearch(page, step) {
		if (await resourceSearch(page).inputValue() !== step.value) throw new Error("Search changed after refresh");
	},
	...sharedActions(["expectResource", "openResource"], async (page, step) => {
		const row = resourceRow(page, step);
		await row.waitFor({ state: "visible" });
		if (await row.count() !== 1) throw new Error("Resource row is ambiguous");
		if (step.action === "openResource") await row.getByRole("button", { name: `Open resource ${step.name}`, exact: true }).click();
	}),
	async expectDetail(page, step) {
		const header = closeDetails(page).locator("xpath=ancestor::header");
		await header.getByRole("heading", { name: step.name, exact: true }).waitFor();
		await header.getByText(`${step.kind} / ${step.namespace}`, { exact: true }).waitFor();
	},
	async closeDetail(page) {
		await closeDetails(page).click();
	},
	async expectDetailClosed(page) {
		await closeDetails(page).waitFor({ state: "detached" });
	},
	async expectEmpty(page) {
		await page.getByText("No resources match current scope and filters.", { exact: true }).waitFor();
		if (await page.locator('tr:has(button[aria-label^="Open resource "])').count()) throw new Error("Rows remain in empty search");
	},
	async refresh(page) {
		await page.getByRole("button", { name: "Live updates and refresh options" }).first().click();
		await page.getByRole("button", { name: "Refresh (clear cache)" }).click();
		await page.getByRole("button", { name: "Refresh (clear cache)" }).waitFor({ state: "visible" });
		await page.waitForFunction(() => {
			const button = [...document.querySelectorAll("button")].find((item) => item.textContent?.trim() === "Refresh (clear cache)");
			return button instanceof HTMLButtonElement && !button.disabled;
		});
		if (await page.getByText("Refresh failed:", { exact: false }).count()) throw new Error("Refresh failed");
		await page.keyboard.press("Escape");
	},
	...sharedActions(["expectClusterResource", "expectNoClusterResource"], async (page, step) => {
		const row = page.locator("tr").filter({ has: page.getByRole("button", { name: `Open resource ${step.name}`, exact: true }) }).filter({ has: page.getByText(step.kind, { exact: true }) });
		// Absence is checked after a preceding step waited for the new scope's rows.
		if (step.action === "expectNoClusterResource") { if (await row.count()) throw new Error(`${step.kind} ${step.name} is still listed`); }
		else await row.first().waitFor({ state: "visible" });
	}),
	// Absence is checked after a preceding step waited for the new scope's rows.
	async expectNoResource(page, step) {
		await resourceRow(page, step).waitFor({ state: "detached" });
	},
} satisfies Partial<WebActions>;

const resourceSearch = (page: Page) => page.getByRole("textbox", { name: "Search resources" });
const closeDetails = (page: Page) => page.getByRole("button", { name: "Close resource details" });
