import { escapeRegExp } from "./web-locators";
import type { WebActions } from "./web-action";
import type { Page } from "playwright-core";
export const rbacActions = {
	async selectRbacCategory(page, step) {
		await rbacCategory(page, step.category).click();
	},
	async expectRbacCategory(page, step) {
		await rbacCategory(page, step.category).and(page.locator('[aria-pressed="true"]')).waitFor();
	},
	async filterRbacRisk(page, step) {
		await page.getByRole("button", { name: new RegExp(`^${escapeRegExp(step.filter)} \\d+$`) }).click();
	},
	async expectRbacItem(page, step) {
		await rbacItem(page, step.title).getByText(step.risk, { exact: true }).waitFor();
	},
	async expectNoRbacItem(page, step) {
		if (await rbacItem(page, step.title).count()) throw new Error(`${step.title} is listed`);
	},
	async selectRbacItem(page, step) {
		await rbacItem(page, step.title).click();
	},
	async investigateRbacSubject(page, step) {
		await page.getByRole("button").filter({ hasText: "Investigate" }).filter({ has: page.locator("span", { hasText: new RegExp(`^${escapeRegExp(step.subject)}$`) }) }).click();
	},
	async expectRbacIdentity(page, step) {
		await rbacSection(page, "Verify exact action").getByText(step.identity, { exact: true }).waitFor();
	},
	async expectRbacGrant(page, step) {
		await rbacGrant(page, step.grant).waitFor();
	},
	async expectNoRbacGrant(page, step) {
		if (await rbacGrant(page, step.grant).count()) throw new Error(`${step.grant} is shown`);
	},
	async verifyRbacAccess(page, step) {
		await page.getByRole("textbox", { name: "Verb", exact: true }).fill(step.verb);
		await page.getByRole("button", { name: "Verify access", exact: true }).click();
	},
	async expectRbacVerifyEnabled(page, step) {
		await page.getByRole("button", { name: "Verify access", exact: true }).and(page.locator(step.enabled ? ":enabled" : ":disabled")).waitFor();
	},
	async expectRbacVerdict(page, step) {
		const label = step.outcome === "noOpinion" ? "No opinion" : step.outcome;
		await rbacSection(page, "Latest verification").locator("p.capitalize", { hasText: new RegExp(`^${label}$`) }).waitFor();
	},
	async editRbacReviewNote(page, step) {
		await page.getByRole("textbox", { name: "RBAC review note", exact: true }).fill(step.value);
	},
	async expectRbacReviewNote(page, step) {
		const input = page.getByRole("textbox", { name: "RBAC review note", exact: true });
		if (await input.inputValue() !== step.value) throw new Error("RBAC review note belongs to another object");
	},
	async saveRbacReview(page, step) {
		await page.getByRole("button", { name: `Mark ${step.disposition}`, exact: true }).click();
	},
	async expectRbacReview(page, step) {
		await page.getByText("Reviewed context", { exact: true }).locator("xpath=ancestor::section[1]").getByText(step.label, { exact: true }).waitFor();
	},
	async clearRbacReview(page) {
		await page.getByRole("button", { name: "Clear review", exact: true }).click();
	},
} satisfies Partial<WebActions>;

function rbacCategory(page: Page, category: string) {
	return page.locator('[aria-label="RBAC categories"]').getByRole("button", { name: new RegExp(`^${escapeRegExp(category)} \\d+$`) });
}
/** A review queue card; its first line is `Kind/name`. */
function rbacItem(page: Page, title: string) {
	return page.locator("button[aria-pressed]").filter({ has: page.locator("p", { hasText: new RegExp(`^${escapeRegExp(title)}$`) }) });
}
function rbacSection(page: Page, heading: string) {
	return page.locator("section").filter({ has: page.getByText(heading, { exact: true }) });
}
function rbacGrant(page: Page, grant: string) {
	return rbacSection(page, "Observed RBAC grants").locator("p.font-mono", { hasText: new RegExp(`^${escapeRegExp(grant)}$`) });
}
