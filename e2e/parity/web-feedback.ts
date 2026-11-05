import type { Page } from "playwright-core";
import type { Step } from "./contract";

const clipboardRecorder = `Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText: async (value) => { document.documentElement.dataset.copyValue = value; } } })`;

export async function feedbackAction(page: Page, step: Step): Promise<void> {
    switch (step.action) {
        case "expectForegroundLoading":
            await page.waitForFunction((active) => document.querySelector("[data-foreground-loading]")?.getAttribute("data-active") === active, step.active ? "true" : "false");
            break;
        case "expectFriendlyError": {
            const alert = page.getByRole("alert").filter({ hasText: step.title });
            await alert.getByText(step.title, { exact: true }).waitFor();
            if (step.next !== undefined) await alert.getByText(step.next, { exact: true }).waitFor();
            if (step.summary !== undefined) await alert.getByText(step.summary, { exact: true }).waitFor();
            break;
        }
        case "inspectRequiredPermission":
            await page.getByRole("alert").getByRole("button", { name: "Inspect required permission", exact: true }).click();
            break;
        case "expectPermissionTarget":
            for (const [name, expected] of [["Verb", step.verb], ["Resource", step.resource]] as const) {
                const field = page.getByRole("textbox", { name, exact: true });
                await field.waitFor();
                const value = await field.inputValue();
                if (value !== expected) throw new Error(`RBAC ${name} is ${JSON.stringify(value)}, expected ${JSON.stringify(expected)}`);
            }
            break;
        case "returnToSource":
            await page.getByRole("button", { name: /^Return to / }).click();
            break;
        case "copyResourceName": {
            const copyName = `Copy resource name: ${step.name}`;
            await page.evaluate(clipboardRecorder);
            await page.getByRole("row").filter({ hasText: step.namespace })
                .filter({ has: page.getByText(step.kind, { exact: true }) })
                .filter({ has: page.getByRole("button", { name: copyName, exact: true }) })
                .getByRole("button", { name: copyName, exact: true })
                .click();
            await page.waitForFunction(() => document.documentElement.dataset.copyValue !== undefined);
            const copied = await page.evaluate(() => document.documentElement.dataset.copyValue);
            if (copied !== step.name) throw new Error(`Copied ${JSON.stringify(copied)}, expected ${JSON.stringify(step.name)}`);
            break;
        }
        case "expectCopyToast":
            await page.getByRole("status").getByText(step.text, { exact: true }).waitFor();
            break;
        case "dismissCopyToast":
            await page.getByRole("button", { name: "Dismiss copy notification", exact: true }).click();
            break;
        default: throw new Error(`Not a feedback action: ${step.action}`);
    }
}
