import type { Page } from "playwright-core";
import type { Step } from "./contract";

export async function sidebarLiveAction(page: Page, step: Step) {
    switch (step.action) {
        case "refreshPage": await page.getByRole("button", { name: "Refresh", exact: true }).click(); break;
        case "replayPageReads": await page.evaluate(`import('/src/lib/tauri-dev-mock-pages.ts').then(m => m.replayPageReads(${JSON.stringify(step.phase)}))`); break;
        case "focusStalePage":
            await page.clock.install();
            await page.evaluate("import('/node_modules/.vite/deps/@tanstack_svelte-query.js').then(m => m.focusManager.setFocused(false))");
            // Query staleTime from the named Svelte surface. No polling is added.
            await page.clock.fastForward(step.page === "RBAC" ? 30_000 : 15_000);
            await page.evaluate("import('/node_modules/.vite/deps/@tanstack_svelte-query.js').then(m => m.focusManager.setFocused(true))");
            break;
        case "expectPageText":
            if (step.text === "Scripted page refresh failure") await page.getByText("Technical detail", { exact: true }).first().click();
            await page.getByText(step.text, { exact: true }).first().waitFor();
            break;
        case "searchCustomKinds": {
            const input = page.getByRole("textbox", { name: "Search custom resources", exact: true });
            if (!(await input.isVisible())) await page.getByRole("button", { name: "Expand Custom Resources", exact: true }).click();
            await input.fill(step.value);
            break;
        }
        case "expectCustomGroup":
            await page.getByRole("button", { name: `Expand ${step.group}`, exact: true }).waitFor({ state: step.visible ? "visible" : "hidden" });
            break;
        case "openKindPicker": await page.getByRole("button", { name: "Edit resource kinds", exact: true }).click(); break;
        case "closeKindPicker": await page.keyboard.press("Escape"); break;
        case "searchKindPicker": await page.getByRole("textbox", { name: "Search resource kinds", exact: true }).fill(step.value); break;
        case "selectAllKinds": await page.getByRole("button", { name: "Select all kinds", exact: true }).click(); break;
        case "toggleKind": case "expectKindChecked": {
            const option = page.locator("label").filter({ has: page.getByText(step.kind, { exact: true }) }).getByRole("checkbox");
            await option.waitFor();
            const checked = await option.getAttribute("aria-checked") === "true";
            if (step.action === "toggleKind") { if (checked !== step.checked) await option.click(); }
            else if (checked !== step.checked) throw new Error(`${step.kind} checked=${checked}, expected ${step.checked}`);
            break;
        }
        default: throw new Error(`Unknown sidebar action ${step.action}`);
    }
}
