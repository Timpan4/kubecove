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
		case "expectYamlApplyDescription": {
			const apply = page.getByRole("button", { name: "Apply", exact: true });
			// The disabled button sits inside the tooltip trigger, which receives the pointer.
			await apply.locator("xpath=..").hover();
			const tooltip = page.locator("#yaml-apply-disabled-tooltip");
			await tooltip.waitFor();
			if ((await tooltip.textContent())?.trim() !== step.text) throw new Error(`Apply hint is not ${step.text}`);
			if ((await apply.getAttribute("aria-describedby")) !== "yaml-apply-disabled-tooltip") throw new Error("Apply is not described by its hint");
			break;
		}
		case "expectNoYamlApplyDescription":
			if ((await page.getByRole("button", { name: "Apply", exact: true }).getAttribute("aria-describedby")) !== null) throw new Error("Apply is still described by a dry-run hint");
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
				// A missing row also has no lock, so the row must be there.
				await page.locator(`[data-resource-action="${step.row}"]`).waitFor();
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
