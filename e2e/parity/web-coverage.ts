import type { WebActions } from "./web-action";
import { checkCoverage } from "./web-coverage-checks";

export const coverageActions = {
	async coverageSelect(page, step) {
		await page.getByRole("combobox", { name: step.label, exact: true }).click();
		await page.getByRole("option", { name: step.value, exact: true }).click();
	},
	async coverageCopy(page, step) {
		await page.evaluate(() => {
			let copied = "";
			Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText: async (text: string) => { copied = text; }, readText: async () => copied } });
		});
		await page.getByRole("button", { name: step.label, exact: true }).first().click();
		const copied = await page.evaluate(() => navigator.clipboard.readText());
		const matches = step.match === "exact" ? copied === step.text : copied.includes(step.text);
		if (!matches) throw new Error("Copy did not use the selected evidence");
	},
	async coverageScript(page, step) {
		if (step.script === "startup-failed" || step.script === "loading") {
			await page.addInitScript((script) => { globalThis.__kubecoveParityCoverage = script; }, step.script);
			await page.reload();
			await page.getByRole("button", { name: "Open settings", exact: true }).waitFor();
		} else await page.evaluate(`import('/src/lib/tauri-dev-mock-coverage.ts').then(m => m.configureCoverage(${JSON.stringify(step.script)}))`);
	},
	async coverageClick(page, step) {
		if (step.label === "Clear Helm search") { await page.getByRole("button", { name: "Clear", exact: true }).click(); return; }
		if (step.label === "5m" || step.label === "Any") { await page.getByRole("radio", { name: step.label, exact: true }).click(); return; }
		if (step.label === "Restart session") await page.getByTitle("Restart session", { exact: true }).click();
		else if (step.label === "Clear") await page.getByRole("tabpanel", { name: "Logs", exact: true }).getByRole("button", { name: "Clear", exact: true }).click();
		else await page.getByRole("button", { name: step.label, exact: true }).first().click();
	},
	async coverageFill(page, step) {
		if (step.label === "Search RBAC objects") await page.getByPlaceholder(/^Search (roles|cluster roles|bindings|service accounts|namespace access)/i).fill(step.value);
		else await page.getByLabel(step.label, { exact: true }).fill(step.value);
	},
	async coverageText(page, step) {
		await page.getByText(step.text, { exact: true }).first().waitFor({ state: step.visible ? "visible" : "hidden" });
	},
	async paletteKey(page, step) {
		await page.keyboard.press(step.key);
	},
	async expectPaletteOpen(page, step) {
		await page.getByPlaceholder("Search views, namespaces, and resources...").waitFor({ state: step.open ? "visible" : "hidden" });
	},
	async deleteSavedWorkspace(page, step) {
		await page.getByRole("button", { name: `Delete ${step.name}`, exact: true }).click();
	},
	async importDecision(page, step) {
		await page.getByRole("radio", { name: step.choice === "Add as new" ? "Add copy" : step.choice, exact: true }).click();
	},
	async cancelImport(page) {
		await page.getByRole("button", { name: "Cancel", exact: true }).click();
	},
	async searchEvents(page, step) {
		await page.getByRole("textbox", { name: "Filter resource events", exact: true }).fill(step.value);
	},
	async coverageCheck(page, step) {
		await checkCoverage(page, step.check);
	},
} satisfies Partial<WebActions>;
