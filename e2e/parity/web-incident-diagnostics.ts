import { escapeRegExp } from "./web-locators";
import type { WebActions } from "./web-action";

/** Incident handoffs, the header update control, and latency report copy. */
export const incidentDiagnosticsActions = {
	async openIncidentResources(page) {
		await page.getByRole("button", { name: "Open resources", exact: true }).click();
	},
	async updateStatusControl(page, step) {
		const dialog = page.getByRole("dialog", { name: "App updates", exact: true });
		if (step.control === "Open") {
			await page.getByRole("button", { name: /^(Check for updates|Update available|Checking for updates|Update check failed|Restart to finish update)$/ }).click();
		} else if (step.control === "Escape") {
			await page.keyboard.press("Escape");
		} else {
			await dialog.getByRole("button", { name: step.control, exact: true }).click();
		}
	},
	async expectUpdateControl(page, step) {
		// The header control is named by its state tooltip, as the GPUI trigger's label is.
		await page.getByRole("button", { name: step.label, exact: true }).waitFor();
	},
	async copyLatencyReport(page) {
		await page.evaluate(() => {
			Object.defineProperty(navigator, "clipboard", { configurable: true, value: {
				writeText: async (value: string) => { document.documentElement.dataset.latencyCopy = value; },
			} });
		});
		await page.getByText("Latency report", { exact: true }).locator("xpath=../..")
			.getByRole("button", { name: "Copy", exact: true }).click();
	},
	async expectLatencyReportCopy(page, step) {
		await page.waitForFunction((expected) => (document.documentElement.dataset.latencyCopy ?? "").includes(expected), step.contains);
	},
	async filterIncidents(page, step) {
		await page.locator('[aria-label="Filter incident signals"]').getByRole("button", { name: new RegExp(`^${escapeRegExp(step.filter)} \\d+$`) }).click();
	},
	async expectIncidentGuide(page, step) {
		await page.getByRole("heading", { level: 3, name: step.title, exact: true }).waitFor();
	},
} satisfies Partial<WebActions>;
