import type { WebActions } from "./web-action";

/** The Kubernetes lint in the YAML editor, and the permission read behind the Actions tab. */
export const yamlPermissionActions = {
	async expectYamlDiagnostic(page, step) {
		const detail = step.path ? `${step.source} - ${step.path}` : step.source;
		// The message sits in a column whose parent is the row with the severity and detail.
		const row = page.getByText(step.message, { exact: true }).locator("xpath=../..");
		await row.getByText(step.severity, { exact: true }).waitFor();
		await row.getByText(detail, { exact: true }).waitFor();
	},
	async expectNoYamlDiagnostic(page, step) {
		await page.getByText(step.message, { exact: true }).waitFor({ state: "detached" });
	},
	async expectYamlApplyDescription(page, step) {
		const apply = page.getByRole("button", { name: "Apply", exact: true });
		// The disabled button sits inside the tooltip trigger, which receives the pointer.
		await apply.locator("xpath=..").hover();
		const tooltip = page.locator("#yaml-apply-disabled-tooltip");
		await tooltip.waitFor();
		if ((await tooltip.textContent())?.trim() !== step.text) throw new Error(`Apply hint is not ${step.text}`);
		if ((await apply.getAttribute("aria-describedby")) !== "yaml-apply-disabled-tooltip") throw new Error("Apply is not described by its hint");
	},
	async expectNoYamlApplyDescription(page) {
		if ((await page.getByRole("button", { name: "Apply", exact: true }).getAttribute("aria-describedby")) !== null) throw new Error("Apply is still described by a dry-run hint");
	},
	async expectActionLoadError(page, step) {
		await page.getByText(step.text, { exact: true }).waitFor();
	},
	async expectNoActionLoadError(page) {
		await page.getByText("Unable to load action state and permissions", { exact: false }).waitFor({ state: "detached" });
	},
	async retryActionPermission(page) {
		await page.getByRole("button", { name: "Retry permission check", exact: true }).click();
	},
	async expectActionLock(page, step) {
		const lock = page.locator(`[data-resource-action="${step.row}"] p`).filter({ hasText: "Needs " });
		if (!step.locked) {
			// A missing row also has no lock, so the row must be there.
			await page.locator(`[data-resource-action="${step.row}"]`).waitFor();
			await lock.waitFor({ state: "detached" });
			return;
		}
		await lock.waitFor();
		if (step.reason) await lock.filter({ hasText: step.reason }).waitFor();
	},
} satisfies Partial<WebActions>;
