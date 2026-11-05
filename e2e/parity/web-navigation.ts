import type { Page } from "playwright-core";
import type { Step } from "./contract";
import { sidebarLiveAction } from "./web-sidebar-live";

export async function navigationAction(page: Page, step: Step): Promise<void> {
    switch (step.action) {
        case "openNamespaceList": await page.getByRole("button", { name: "View all", exact: true }).click(); break;
        case "expectNamespaceRow": {
            const row = page.getByRole("row").filter({ has: page.getByRole("cell", { name: step.name, exact: true }) });
            await row.getByRole("cell", { name: step.age, exact: true }).first().waitFor();
            break;
        }
        case "resizeViewport": await page.setViewportSize({ width: step.width, height: step.height }); break;
        case "openNavigationDrawer": await page.getByRole("button", { name: "Open workspace navigation", exact: true }).click(); break;
        case "expectNavigationDrawer":
            await page.getByRole("dialog", { name: "Workspace navigation", exact: true }).waitFor({ state: step.open ? "visible" : "hidden" });
            break;
        case "sidebarKey":
            // A click focuses the row, as a pointer user does before pressing keys.
            await sidebarLiveAction(page, { action: "sidebarSelect", path: step.path });
            await page.keyboard.press(step.key);
            break;
        default: throw new Error(`Unknown navigation action ${step.action}`);
    }
}
