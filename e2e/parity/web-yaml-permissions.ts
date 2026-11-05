import type { Page } from "playwright-core";
import type { Step } from "./contract";

/** The Kubernetes lint in the YAML editor, and the permission read behind the Actions tab. */
export async function yamlPermissionAction(page: Page, step: Step) {
	switch (step.action) {
		case "expectYamlDiagnostic": {
			const detail = step.path ? `${step.source} - ${step.path}` : step.source;
			// The message sits in a column whose parent is the row with the severity and detail.
			const row = page.getByText(step.message, { exact: true }).locator("xpath=../..");
			await row.getByText(step.severity, { exact: true }).waitFor();
			await row.getByText(detail, { exact: true }).waitFor();
			break;
		}
		case "expectNoYamlDiagnostic":
			await page.getByText(step.message, { exact: true }).waitFor({ state: "detached" });
			break;
		case "expectActionLoadError":
			await page.getByText(step.text, { exact: true }).waitFor();
			break;
		case "expectNoActionLoadError":
			await page.getByText("Unable to load action state and permissions", { exact: false }).waitFor({ state: "detached" });
			break;
		case "retryActionPermission":
			await page.getByRole("button", { name: "Retry permission check", exact: true }).click();
			break;
		case "expectActionLock": {
			const lock = page.locator(`[data-resource-action="${step.row}"] p`).filter({ hasText: "Needs " });
			if (!step.locked) {
				await lock.waitFor({ state: "detached" });
				break;
			}
			await lock.waitFor();
			if (step.reason) await lock.filter({ hasText: step.reason }).waitFor();
			break;
		}
		default: throw new Error(`Unknown YAML permission action ${step.action}`);
	}
}
