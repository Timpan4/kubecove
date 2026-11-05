import type { Page } from "playwright-core";
import type { Step } from "./contract";

export async function terminalAction(page: Page, step: Step): Promise<void> {
    const terminal = page.locator(".xterm:visible");
    switch (step.action) {
        case "terminalFit":
            await page.setViewportSize({ width: step.width, height: step.height });
            await page.evaluate(() => new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve()))));
            break;
        case "terminalOutput":
            await page.evaluate(`import('/src/lib/tauri-dev-mock-session-controls.ts').then(m => { for (const session of m.listMockExecs()) { if (session.container === ${JSON.stringify(step.container)}) m.mockExecInput(session.id, ${JSON.stringify(step.data)}); } })`);
            // Flush xterm's asynchronous write queue through its rendered rows.
            await page.evaluate(() => new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve()))));
            break;
        case "terminalSelect": {
            const screen = terminal.locator(".xterm-screen");
            const bounds = await screen.boundingBox();
            const row = await terminal.locator(".xterm-rows > div").first().boundingBox();
            const width = await terminal.locator(".xterm-rows").evaluate((element) => {
                const context = document.createElement("canvas").getContext("2d");
                if (!context) throw new Error("No canvas text measurement");
                context.font = getComputedStyle(element).font;
                return context.measureText("W").width;
            });
            if (!bounds || !row || !width) throw new Error("Terminal cell geometry unavailable");
            const x = bounds.x + (step.col + 0.1) * width;
            const y = bounds.y + (step.row + 0.5) * row.height;
            if (step.word) await page.mouse.dblclick(x, y);
            else {
                await page.mouse.move(x, y);
                await page.mouse.down();
                await page.mouse.move(bounds.x + (step.endCol + 0.1) * width, bounds.y + (step.endRow + 0.5) * row.height);
                await page.mouse.up();
            }
            break;
        }
        case "terminalCopy": {
            // Exercise xterm's real copy listener without touching OS clipboard.
            const copied = await terminal.evaluate((element) => {
                const clipboardData = new DataTransfer();
                element.dispatchEvent(new ClipboardEvent("copy", { bubbles: true, cancelable: true, clipboardData }));
                return clipboardData.getData("text/plain");
            });
            if (copied !== step.text) throw new Error(`Terminal copied ${JSON.stringify(copied)}, expected ${JSON.stringify(step.text)}`);
            break;
        }
        case "terminalPaste":
            await terminal.locator("textarea").evaluate((element, text) => {
                const clipboardData = new DataTransfer();
                clipboardData.setData("text/plain", text);
                element.dispatchEvent(new ClipboardEvent("paste", { bubbles: true, cancelable: true, clipboardData }));
            }, step.text);
            break;
        case "terminalScroll":
            await terminal.locator(".xterm-viewport").evaluate((element, rows) => {
                const row = element.parentElement?.querySelector(".xterm-rows > div");
                if (!row) throw new Error("No terminal rows");
                element.scrollTop -= rows * row.getBoundingClientRect().height;
            }, step.rows);
            await page.evaluate(() => new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve()))));
            break;
    }
}
