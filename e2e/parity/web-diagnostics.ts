import type { Page } from "playwright-core";
import type { Step } from "./contract";

/** Latency report view timings and the interaction recorder's controls. */
export async function diagnosticsAction(page: Page, step: Step): Promise<void> {
    switch (step.action) {
        case "expectViewTiming": {
            // The web times each view's data request as `<view>.fetch.done`; GPUI times `<view>.present`.
            const row = page.getByRole("row").filter({ has: page.getByRole("cell", { name: `${step.view}.fetch.done`, exact: true }) });
            await row.waitFor();
            if (!(await row.textContent())?.includes("ms")) throw new Error("Missing measured duration");
            break;
        }
        case "copyInteractions": case "clearInteractions":
            if (step.action === "copyInteractions") {
                await page.evaluate(() => {
                    Object.defineProperty(navigator, "clipboard", { configurable: true, value: {
                        writeText: async (value: string) => { document.documentElement.dataset.interactionCopy = value; },
                    } });
                });
            }
            await page.getByText("Interaction report", { exact: true }).locator("xpath=../..")
                .getByRole("button", { name: step.action === "copyInteractions" ? "Copy" : "Clear", exact: true }).click();
            break;
        case "expectInteractionCopy": {
            const copied = await (await page.waitForFunction(() => document.documentElement.dataset.interactionCopy ?? "")).jsonValue();
            if (step.contains !== undefined && !copied.includes(step.contains)) throw new Error(`Interaction copy lacks ${step.contains}`);
            if (step.excludes !== undefined && copied.includes(step.excludes)) throw new Error(`Interaction copy includes ${step.excludes}`);
            break;
        }
        case "typeSettingsField": {
            const field = page.getByLabel(step.field, { exact: true });
            await field.click();
            await field.pressSequentially(step.value);
            break;
        }
        default: throw new Error(`Not a diagnostics action: ${step.action}`);
    }
}
