import type { Page } from "playwright-core";
import type { Step } from "./contract";

// One-line log rows are `[data-log-row]` buttons. Their headline is an exact text node.
function logRow(page: Page, line: string) {
    return page.locator('[data-log-row][role="button"]').filter({ has: page.getByText(line, { exact: true }) });
}

function restorePrompt(page: Page) {
    return page.getByRole("alert").filter({ hasText: "Saved port forwards available" });
}

export async function logRowAndRestoreAction(page: Page, step: Step): Promise<void> {
    switch (step.action) {
        case "toggleLogRow":
            await logRow(page, step.line).getByText(step.line, { exact: true }).click();
            break;
        case "toggleLogExpandAll":
            await page.getByRole("button", { name: /^(Expand|Collapse) all$/ }).click();
            break;
        case "expectLogRowOpen": {
            const row = logRow(page, step.line);
            await row.waitFor();
            if ((await row.getAttribute("aria-expanded")) !== String(step.open)) throw new Error(`Log row open state differs: ${step.line}`);
            break;
        }
        case "copyLogLine":
            await page.evaluate(() => Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText: async (value: string) => { document.documentElement.dataset.logCopy = value; } } }));
            await logRow(page, step.line).getByRole("button", { name: "Copy line", exact: true }).click();
            if (!(await page.locator("html").getAttribute("data-log-copy"))?.includes(step.contains)) throw new Error("Wrong log clipboard value");
            break;
        case "expectRestorePrompt": {
            const prompt = restorePrompt(page);
            if (!step.visible) {
                await prompt.waitFor({ state: "hidden" });
                break;
            }
            await prompt.waitFor();
            if (step.count !== undefined) {
                const text = `This workspace has ${step.count} saved Service ${step.count === 1 ? "forward" : "forwards"} ready to start.`;
                if (!(await prompt.textContent())?.includes(text)) throw new Error("Wrong restore prompt count");
            }
            break;
        }
        case "restorePromptControl": {
            const prompt = restorePrompt(page);
            if (step.control === "Auto-start") await prompt.getByRole("checkbox", { name: "Auto-start saved port forwards", exact: true }).click();
            else await prompt.getByRole("button", { name: step.control, exact: true }).click();
            break;
        }
    }
}
