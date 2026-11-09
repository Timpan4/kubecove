import { isDeepStrictEqual } from "node:util";
import type { Page } from "playwright-core";
import original from "../../src/lib/settings-integration-workspace.json";
import type { Step } from "./contract";

export function expectRetainedWorkspaceFields(items: { spec: { displayName: string; scope: typeof original.spec.scope; portForwards: unknown } }[], name: string) {
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
        case "expectContextUnavailable":
            await page.getByRole("group", { name: `${step.name} actions`, exact: true }).locator('xpath=ancestor::*[@data-slot="card"]').getByText("Context unavailable", { exact: true }).waitFor(); break;
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
        case "exportSavedWorkspace":
            await page.getByRole("button", { name: `Export ${step.name}`, exact: true }).click();
            await page.getByText(`Exported ${step.name}.`, { exact: true }).waitFor(); break;
        case "expectExportedShortcut": {
            const content = String(await page.evaluate("import('/src/lib/settings-integration-mocks.ts').then(m => m.workspaceExportMock())"));
            const document = JSON.parse(content);
            const items: { spec: { displayName: string; shortcuts: unknown[] } }[] = document.kind === "WorkspaceList" ? document.items : [document];
            const shortcuts = items.find((item) => item.spec.displayName === step.name)?.spec.shortcuts ?? [];
            if (!shortcuts.some((shortcut) => isDeepStrictEqual(shortcut, step.shortcut))) throw new Error(`Export of ${step.name} lacks shortcut ${JSON.stringify(step.shortcut)}: ${JSON.stringify(shortcuts)}`);
            break;
        }
        // The detail header names the target context: a badge with several contexts, screen-reader text with one.
        case "expectDetailContext":
            await page.getByRole("button", { name: "Close resource details", exact: true }).locator("xpath=ancestor::header")
                .getByText(`Context: ${step.context}`, { exact: true }).waitFor(); break;
        default: throw new Error(`Not a workspace action: ${step.action}`);
    }
}
