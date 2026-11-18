import { escapeRegExp } from "./web-locators";
import type { WebActions } from "./web-action";
import { gitOpsDetails, helmCard, revealGitOps } from "./web-delivery-helpers";
import type { Page } from "playwright-core";

/** The group headers of the GitOps list, in list order. */
function gitOpsGroups(page: Page) { return page.locator("[data-gitops-group]"); }

/** GitOps list steps. */
export const deliveryActions = {
	async expectGitOpsGroup(page, step) {
		const group = gitOpsGroups(page)
			.and(page.getByRole("button", { name: `${step.label} ${step.count}`, exact: true }))
			.and(page.locator(`[data-gitops-group-position="${step.position}"][aria-expanded="${String(step.expanded)}"]`));
		await revealGitOps(page, group);
		await group.waitFor();
	},
	// "Healthy" is part of "Not healthy", so the label must start the header's name.
	async toggleGitOpsGroup(page, step) {
		const group = gitOpsGroups(page).and(page.getByRole("button", { name: new RegExp(`^${escapeRegExp(step.label)} \\d+$`) }));
		await revealGitOps(page, group);
		await group.click();
	},
	// The Helm frame header's context badge, which carries screen-reader text before the name.
	async expectHelmContext(page, step) {
		await page.getByRole("heading", { level: 2, name: "Helm", exact: true }).locator("xpath=ancestor::header")
			.getByText(`Context: ${step.context}`, { exact: true }).waitFor();
	},
	async expectGitOpsCount(page, step) {
		await page.locator("div.justify-between").filter({ has: page.getByText(step.label, { exact: true }) }).locator("strong").filter({ hasText: new RegExp(`^${step.count}$`) }).waitFor();
	},
	async openGitOps(page) {
		await page.getByRole("button", { name: "Open GitOps", exact: true }).scrollIntoViewIfNeeded();
		await page.getByRole("button", { name: "Open GitOps", exact: true }).click();
	},
	async openGitOpsResources(page, step) {
		await revealGitOps(page, gitOpsDetails(page, step));
		const resources = page.getByRole("button", { name: `View resources for ${step.name}`, exact: true });
		// Same-named Applications in other namespaces share the label; the deepest
		// element holding both it and the step's Details button is the step's card.
		const target = await resources.count() > 1
			? page.locator("div").filter({ has: resources }).filter({ has: gitOpsDetails(page, step) }).last().getByRole("button", { name: `View resources for ${step.name}`, exact: true })
			: resources.first();
		await target.click();
	},
	async expectArgoWorkspace(page, step) {
		await page.getByRole("heading", { level: 2 }).filter({ hasText: step.name }).waitFor();
	},
	async filterArgoResources(page, step) {
		await page.getByLabel("Argo CD resource filters").getByRole("button", { name: new RegExp(`^${escapeRegExp(step.filter)} `) }).click();
	},
	async expectDeliveryView(page, step) {
		await page.getByRole("radio", { name: step.mode, exact: true }).and(page.locator('[aria-checked="true"]')).waitFor();
	},
	async setDeliveryView(page, step) {
		await page.getByRole("radio", { name: step.mode, exact: true }).click();
	},
	async expectGitOpsTable(page, step) {
		await page.getByRole("heading", { level: 2, name: step.title, exact: true }).waitFor();
		await page.getByText(itemCount(step.count), { exact: true }).waitFor();
	},
	async expectGitOpsItem(page, step) {
		await gitOpsDetails(page, step).waitFor();
	},
	// Absence is checked after a preceding step waited for the page's rows.
	async expectNoGitOpsItem(page, step) {
		if (await gitOpsDetails(page, step).count()) throw new Error(`${step.kind} ${step.name} is still listed`);
	},
	async openGitOpsDetails(page, step) {
		const button = gitOpsDetails(page, step);
		await revealGitOps(page, button);
		await button.click();
	},
	async expectArgoApplication(page, step) {
		const panel = page.getByRole("tabpanel");
		await panel.getByRole("heading", { level: 2, name: step.name, exact: true }).waitFor();
		await panel.getByText(`Health ${step.health}`, { exact: true }).waitFor();
		await panel.getByText(`Sync ${step.sync}`, { exact: true }).waitFor();
	},
	async expectArgoSignal(page, step) {
		await page.getByRole("tabpanel").locator("div.p-2\\.5").filter({ has: page.getByText(step.label, { exact: true }) }).locator("div.text-base").filter({ hasText: new RegExp(`^${step.count}$`) }).waitFor();
	},
	async expectHelmCount(page, step) {
		await page.getByText(itemCount(step.count), { exact: true }).waitFor();
	},
	async expectHelmRelease(page, step) {
		await helmCard(page, step.name, step.namespace).waitFor();
	},
	async expectNoHelmRelease(page, step) {
		if (await helmCard(page, step.name, step.namespace).count()) throw new Error(`Helm release ${step.name} is still listed`);
	},
	async openHelmRelease(page, step) {
		await helmCard(page, step.name, step.namespace).getByRole("button", { name: "Details", exact: true }).click();
	},
	async selectHelmNamespace(page, step) {
		await page.getByRole("button", { name: new RegExp(`^${escapeRegExp(step.namespace)}\\s*\\d+$`) }).click();
	},
	async searchHelm(page, step) {
		await page.getByRole("textbox", { name: "Search Helm releases" }).fill(step.value);
	},
	async expectHelmDetails(page, step) {
		const card = page.locator('[data-slot="card"]').filter({ has: page.getByRole("button", { name: "Close Helm details", exact: true }) });
		await card.locator('[data-slot="card-title"]').getByText(step.title, { exact: true }).waitFor();
		await card.getByText(step.description, { exact: true }).waitFor();
		await card.getByText(step.totals, { exact: true }).waitFor();
	},
} satisfies Partial<WebActions>;

function itemCount(count: number) { return `${count} item${count === 1 ? "" : "s"}`; }
