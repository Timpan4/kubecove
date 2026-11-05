import type { Page } from "playwright-core";
import type { Step } from "./contract";
import original from "../../src/lib/settings-integration-workspace.json";
import { isDeepStrictEqual } from "node:util";

export function expectRetainedWorkspaceFields(items: { spec: { displayName: string; scope: Record<string, unknown>; portForwards: unknown } }[], name: string) {
    const item = items.find((item) => item.spec.displayName === name);
    if (!item) throw new Error("Missing exported workspace");
    for (const field of ["kinds", "layout", "gitOpsFilter", "argoAppFilter", "shortcutPreferences"] as const) {
        if (!isDeepStrictEqual(item.spec.scope[field], original.spec.scope[field])) throw new Error(`Workspace lost ${field}`);
    }
    if (!isDeepStrictEqual(item.spec.portForwards, original.spec.portForwards)) throw new Error("Workspace lost saved forwards");
}

export async function workspaceAction(page: Page, step: Step): Promise<void> {
    switch (step.action) {
        case "failWorkspaceContext":
            await page.evaluate(`import('/src/lib/tauri-dev-mocks.ts').then(m => m.failWorkspaceContext(${JSON.stringify(step.context)}))`); break;
        case "expectWorkspaceReadWarning":
            await page.getByRole("alert").filter({ hasText: step.context }).filter({ hasText: "Context unavailable" }).waitFor(); break;
        case "editSavedWorkspace":
            await page.getByRole("button", { name: `Edit ${step.name}`, exact: true }).click(); break;
        case "expectNoSavedWorkspace":
            await page.getByRole("button", { name: `Open ${step.name}`, exact: true }).waitFor({ state: "hidden" }); break;
        case "saveWorkspace":
            await page.getByRole("button", { name: "Save workspace", exact: true }).click();
            await page.getByText("New workspace", { exact: true }).waitFor(); break;
        case "cancelWorkspaceEdit":
            await page.getByRole("button", { name: "Cancel edit", exact: true }).click(); break;
        case "toggleWorkspaceContext":
            await page.getByRole("checkbox", { name: step.context, exact: true }).click(); break;
        case "expectOverviewHealth": {
            const card = page.locator('[data-slot="card"]').filter({ has: page.getByText("Resource Health", { exact: true }) }).first();
            await card.getByText("Total", { exact: true }).locator("..").locator("strong").filter({ hasText: new RegExp(`^${step.total}$`) }).waitFor();
            break;
        }
        case "expectDetailContext":
            await page.getByRole("tab", { name: "YAML", exact: true }).click();
            await page.getByRole("button", { name: "Edit YAML", exact: true }).click();
            await page.getByRole("tabpanel").getByText(`Context: ${step.context}`, { exact: true }).waitFor();
            await page.getByRole("button", { name: "Cancel", exact: true }).click(); break;
        default: throw new Error(`Not a workspace action: ${step.action}`);
    }
}
