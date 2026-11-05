import type { Page } from "playwright-core";
import type { Step } from "./contract";

/** Incident handoffs, the header update control, and latency report copy. */
export async function incidentDiagnosticsAction(page: Page, step: Step): Promise<void> {
    switch (step.action) {
        case "openIncidentResources":
            await page.getByRole("button", { name: "Open resources", exact: true }).click();
            break;
        case "expectHealthFilter": {
            const stat = page.getByLabel("Resource health summary", { exact: true })
                .getByRole("button")
                .filter({ has: page.locator("span", { hasText: new RegExp(`^${step.label}$`) }) });
            if (await stat.getAttribute("aria-pressed") !== "true") throw new Error(`Wrong health filter: ${step.label}`);
            break;
        }
        case "updateStatusControl": {
            const dialog = page.getByRole("dialog", { name: "App updates", exact: true });
            if (step.control === "Open") {
                await page.getByRole("button", { name: /^(Check for updates|Update available|Checking for updates|Update check failed)$/ }).click();
            } else if (step.control === "Escape") {
                await page.keyboard.press("Escape");
            } else {
                await dialog.getByRole("button", { name: step.control, exact: true }).click();
            }
            break;
        }
        case "copyLatencyReport":
            await page.evaluate(() => {
                Object.defineProperty(navigator, "clipboard", { configurable: true, value: {
                    writeText: async (value: string) => { document.documentElement.dataset.latencyCopy = value; },
                } });
            });
            await page.getByText("Latency report", { exact: true }).locator("xpath=../..")
                .getByRole("button", { name: "Copy", exact: true }).click();
            break;
        case "expectLatencyReportCopy":
            await page.waitForFunction((expected) => (document.documentElement.dataset.latencyCopy ?? "").includes(expected), step.contains);
            break;
        default: throw new Error(`Not an incident diagnostics action: ${step.action}`);
    }
}
