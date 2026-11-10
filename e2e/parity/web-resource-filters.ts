import type { Page } from "playwright-core";
import type { Step } from "./contract";

const escapeRegExp = (value: string) => value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
const healthSummary = (page: Page) => page.locator('[aria-label="Resource health summary"]');
const healthButton = (page: Page, label: string) => healthSummary(page).getByRole("button", { name: new RegExp(`^${escapeRegExp(label)}\\b`) });
const namespaceTrigger = (page: Page) => page.getByRole("button", { name: "Edit namespace scope", exact: true });

export async function resourceFilterAction(page: Page, step: Step) {
    switch (step.action) {
        case "openNamespacePicker": await namespaceTrigger(page).click(); break;
        case "closeNamespacePicker": await page.keyboard.press("Escape"); break;
        case "searchNamespacePicker": await page.getByRole("textbox", { name: "Search namespaces", exact: true }).fill(step.value); break;
        case "selectAllNamespaces": await page.getByRole("button", { name: "Select all namespaces", exact: true }).click(); break;
        case "toggleNamespace": case "expectNamespaceChecked": {
            const option = page.locator("label").filter({ has: page.getByText(step.namespace, { exact: true }) }).getByRole("checkbox");
            await option.waitFor();
            const checked = await option.getAttribute("aria-checked") === "true";
            if (step.action === "toggleNamespace") { if (checked !== step.checked) await option.click(); }
            else if (checked !== step.checked) throw new Error(`${step.namespace} checked=${checked}, expected ${step.checked}`);
            break;
        }
        case "expectNamespaceTrigger": await namespaceTrigger(page).getByText(step.label, { exact: true }).waitFor(); break;
        case "selectHealthFilter": await healthButton(page, step.label).click(); break;
        case "expectHealthFilter": await healthButton(page, step.label).and(page.locator(`[aria-pressed="${step.pressed}"]`)).waitFor(); break;
        case "chooseGitOpsOwner":
            await page.getByRole("combobox").filter({ hasText: /GitOps owners|Owned by/ }).click();
            await page.getByRole("option", { name: step.owner, exact: true }).click();
            break;
        case "clearResourceFilters": await page.getByRole("button", { name: "Clear", exact: true }).click(); break;
        case "setHistoricalReplicaSets": await page.getByRole("checkbox", { name: "Hide historical ReplicaSets", exact: true }).setChecked(step.checked); break;
        default: throw new Error("Unknown resource filter action");
    }
}
