import { escapeRegExp } from "./web-locators";
import type { WebActions } from "./web-action";
import { revisionButton } from "./web-delivery-helpers";
import { expectDetailScroll, expectResourceEvent } from "./web-detail-scroll";
import type { Locator, Page } from "playwright-core";
/** Details metadata controls are matched by visible text, the same text the native driver labels. */
export const detailsActions = {
	async clickDetailControl(page, step) {
		await page.getByText(step.label, { exact: true }).first().click();
	},
	async showYaml(page) {
		await page.getByRole("tab", { name: "YAML", exact: true }).click();
	},
	async expectYaml(page, step) {
		for (const fragment of step.contains) await page.getByText(fragment, { exact: false }).first().waitFor();
	},
	async showTab(page, step) {
		await page.getByRole("tab", { name: step.tab, exact: true }).click();
	},
	async filterEvents(page, step) {
		await page.getByRole("radiogroup", { name: "Filter events by type" }).getByRole("radio", { name: step.filter, exact: true }).click();
	},
	async expectDetailScroll(page, step) {
		await expectDetailScroll(page, step.view);
	},
	async expectEventCount(page, step) {
		if (step.count) await eventRows(page).nth(step.count - 1).waitFor();
		await eventRows(page).nth(step.count).waitFor({ state: "detached" });
	},
	async expectEvent(page, step) {
		await expectResourceEvent(page, step);
	},
	async selectRevision(page, step) {
		await revisionButton(page, step.label).click();
	},
	async expectRevisionDiff(page, step) {
		const header = page.locator("div.border-b").filter({ hasText: "compared with" });
		await header.locator("span.font-medium", { hasText: new RegExp(`^${escapeRegExp(step.selected)}$`) }).waitFor();
		await header.getByRole("combobox", { name: "Comparison revision" }).filter({ hasText: new RegExp(`^\\s*${escapeRegExp(step.comparison)}\\s*$`) }).waitFor();
		await containsLines(page.getByRole("tabpanel").locator("pre"), step.contains);
	},
	async reviewAction(page, step) {
		await page.getByRole("button", { name: step.label, exact: true }).click();
	},
	async editReplicas(page, step) {
		await page.locator('[data-resource-action="replicas"]').getByRole("button", { name: "Edit", exact: true }).click();
		await page.getByRole("spinbutton", { name: "Desired replicas" }).fill(step.value);
	},
	async expectReview(page, step) {
		const dialog = page.getByRole("dialog");
		await dialog.getByRole("heading", { name: step.title, exact: true }).waitFor();
		await dialog.getByText(step.description, { exact: true }).waitFor();
		await containsLines(dialog.getByLabel("Server result diff"), step.contains);
	},
	// Forced, so a disabled control is clicked like a user would and must do nothing.
	async executeReview(page) {
		await page.getByRole("dialog").getByRole("button", { name: /^(Apply|Delete \w+)$/ }).click({ force: true });
	},
	async closeReview(page) {
		await page.getByRole("dialog").getByRole("button", { name: "Cancel", exact: true }).click();
	},
	async expectNoReview(page) {
		await page.getByRole("dialog").waitFor({ state: "detached" });
	},
	async expectActionResult(page, step) {
		await page.getByRole("status").filter({ hasText: new RegExp(`^${escapeRegExp(step.text)}$`) }).waitFor();
	},
	async expectNoActionResult(page) {
		if (await page.locator('p[role="status"].text-emerald-300').count()) throw new Error("An action result is shown");
	},
	async editYaml(page) {
		await page.getByRole("button", { name: "Edit YAML", exact: true }).click({ force: true });
	},
	async applyYaml(page) {
		await page.getByRole("button", { name: "Apply", exact: true }).click({ force: true });
	},
	async dryRunYaml(page) {
		await page.getByRole("button", { name: "Dry run", exact: true }).click();
	},
	async expectDryRun(page, step) {
		await containsLines(page.locator("div.font-mono.text-xs.leading-relaxed"), step.contains);
	},
	async expectYamlNotice(page, step) {
		await page.getByText(step.title, { exact: true }).first().waitFor();
	},
	async expectNoYamlNotice(page, step) {
		if (await page.getByText(step.title, { exact: true }).count()) throw new Error(`${step.title} is shown`);
	},
	async expectYamlReadOnly(page) {
		await page.getByRole("button", { name: "Edit YAML", exact: true }).waitFor();
		if (await page.getByRole("button", { name: "Dry run", exact: true }).count()) throw new Error("The YAML draft editor opened");
	},
} satisfies Partial<WebActions>;

/** Event rows in the Events tab: each starts with its `<time>`. */
function eventRows(page: Page) { return page.locator("div.grid:has(> time[datetime])"); }

/** Waits until the element's rendered lines include every expected diff line. */
async function containsLines(locator: Locator, lines: string[]) {
	const element = await locator.first().elementHandle();
	await locator.page().waitForFunction(({ element, expected }) => {
		// biome-ignore lint/performance/useDomNodeTextContent: the YAML dry-run diff renders each line as a block <div>, so only rendered text keeps one line per row.
		const rendered = element instanceof HTMLElement ? element.innerText.split("\n") : [];
		return expected.every((line) => rendered.includes(line));
	}, { element, expected: lines });
}
