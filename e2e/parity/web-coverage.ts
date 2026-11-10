import type { Page } from "playwright-core";
import type { Step } from "./contract";

export async function coverageAction(page: Page, step: Step) {
	switch (step.action) {
		case "coverageSelect":
			await page.getByRole("combobox", { name: step.label, exact: true }).click();
			await page.getByRole("option", { name: step.value, exact: true }).click();
			break;
		case "coverageCopy": {
			await page.evaluate(() => {
				let copied = "";
				Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText: async (text: string) => { copied = text; }, readText: async () => copied } });
			});
			await page.getByRole("button", { name: step.label, exact: true }).first().click();
			const copied = await page.evaluate(() => navigator.clipboard.readText());
			const matches = step.match === "exact" ? copied === step.text : copied.includes(step.text);
			if (!matches) throw new Error("Copy did not use the selected evidence");
			break;
		}
        case "coverageScript":
            if (step.script === "startup-failed" || step.script === "loading") {
                await page.addInitScript((script) => { globalThis.__kubecoveParityCoverage = script; }, step.script);
                await page.reload();
                await page.getByRole("button", { name: "Open settings", exact: true }).waitFor();
            } else await page.evaluate(`import('/src/lib/tauri-dev-mock-coverage.ts').then(m => m.configureCoverage(${JSON.stringify(step.script)}))`);
            break;
		case "coverageClick":
			if (step.label === "Clear Helm search") { await page.getByRole("button", { name: "Clear", exact: true }).click(); break; }
			if (step.label === "5m" || step.label === "Any") { await page.getByRole("radio", { name: step.label, exact: true }).click(); break; }
			if (step.label === "Restart session") await page.getByTitle("Restart session", { exact: true }).click();
			else if (step.label === "Clear") await page.getByRole("tabpanel", { name: "Logs", exact: true }).getByRole("button", { name: "Clear", exact: true }).click();
			else await page.getByRole("button", { name: step.label, exact: true }).first().click();
			break;
		case "coverageFill":
			if (step.label === "Search RBAC objects") await page.getByPlaceholder(/^Search (roles|cluster roles|bindings|service accounts|namespace access)/i).fill(step.value);
			else await page.getByLabel(step.label, { exact: true }).fill(step.value);
			break;
		case "coverageText":
			await page.getByText(step.text, { exact: true }).first().waitFor({ state: step.visible ? "visible" : "hidden" });
			break;
		case "paletteKey":
			await page.keyboard.press(step.key);
			break;
		case "expectPaletteOpen":
			await page.getByPlaceholder("Search views, namespaces, and resources...").waitFor({ state: step.open ? "visible" : "hidden" });
			break;
		case "deleteSavedWorkspace":
			await page.getByRole("button", { name: `Delete ${step.name}`, exact: true }).click();
			break;
		case "importDecision":
			await page.getByRole("radio", { name: step.choice === "Add as new" ? "Add copy" : step.choice, exact: true }).click();
			break;
		case "cancelImport":
			await page.getByRole("button", { name: "Cancel", exact: true }).click();
			break;
		case "searchEvents":
			await page.getByRole("textbox", { name: "Filter resource events", exact: true }).fill(step.value);
			break;
		default: throw new Error(`Unknown coverage action ${step.action}`);
	}
}
