import type { Locator, Page } from "playwright-core";
import type { Step } from "./contract";

const frames = (page: Page) => page.evaluate(() => new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve()))));

/** The rows' box and one cell's size, as xterm's DOM renderer lays them out. */
async function geometry(terminal: Locator) {
    const bounds = await terminal.locator(".xterm-screen").boundingBox();
    const row = await terminal.locator(".xterm-rows > div").first().boundingBox();
    // xterm sizes the screen to whole cells. Its width-cache spans repeat one
    // monospace character, the first in the regular font.
    const measured = await terminal.locator(".xterm-char-measure-element").first().evaluate((element) => element.getBoundingClientRect().width / (element.textContent?.length || 1));
    if (!bounds || !row || !measured) throw new Error("Terminal cell geometry unavailable");
    const width = bounds.width / Math.round(bounds.width / measured);
    const rows = await terminal.locator(".xterm-rows > div").count();
    return { bounds, width, height: row.height, rows, at: (r: number, c: number) => ({ x: bounds.x + (c + 0.1) * width, y: bounds.y + (r + 0.5) * row.height }) };
}

export async function terminalAction(page: Page, step: Step): Promise<void> {
    const terminal = page.locator(".xterm:visible");
    switch (step.action) {
        case "terminalFit":
            await page.setViewportSize({ width: step.width, height: step.height });
            await frames(page);
            break;
        case "terminalOutput":
            await page.evaluate(`import('/src/lib/tauri-dev-mock-session-controls.ts').then(m => { for (const session of m.listMockExecs()) { if (session.container === ${JSON.stringify(step.container)}) m.mockExecInput(session.id, ${JSON.stringify(step.data)}); } })`);
            // Flush xterm's asynchronous write queue through its rendered rows.
            await frames(page);
            break;
        case "terminalSelect": {
            const cells = await geometry(terminal);
            const from = cells.at(step.row, step.col);
            if (step.word) await page.mouse.dblclick(from.x, from.y);
            else if (step.line) await page.mouse.click(from.x, from.y, { clickCount: 3 });
            else {
                if (step.alt) await page.keyboard.down("Alt");
                await page.mouse.move(from.x, from.y);
                await page.mouse.down();
                const to = cells.at(step.endRow, step.endCol);
                await page.mouse.move(to.x, to.y);
                await page.mouse.up();
                if (step.alt) await page.keyboard.up("Alt");
            }
            break;
        }
        case "terminalDragOutside": {
            const cells = await geometry(terminal);
            const from = cells.at(step.row, step.col);
            const viewport = page.viewportSize();
            if (!viewport) throw new Error("No page viewport");
            // xterm scrolls faster up to 50 px past the rows. Stay inside the page so the
            // browser still delivers the moves.
            const top = cells.bounds.y;
            const bottom = top + cells.rows * cells.height;
            const room = step.edge === "below" ? viewport.height - 1 - bottom : top - 1;
            if (room < 1) throw new Error(`No room ${step.edge} the terminal rows`);
            const distance = Math.min(50, room);
            await page.mouse.move(from.x, from.y);
            await page.mouse.down();
            await page.mouse.move(from.x, step.edge === "below" ? bottom + distance : top - distance);
            break;
        }
        case "terminalRelease":
            // xterm's drag scroll runs on a real 50 ms interval.
            await page.waitForTimeout(step.ms);
            await page.mouse.up();
            await frames(page);
            break;
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
        case "terminalPrimaryPaste": {
            if (process.platform !== "linux") throw new Error("Primary selection paste is Linux-only");
            // xterm's onLinuxMouseSelection selects the mouse selection in its textarea;
            // WebKitGTK owns that selection as PRIMARY.
            const primary = await terminal.locator("textarea").evaluate((element: HTMLTextAreaElement) => element.value.slice(element.selectionStart, element.selectionEnd));
            if (primary !== step.text) throw new Error(`Primary selection is ${JSON.stringify(primary)}, expected ${JSON.stringify(step.text)}`);
            const box = await terminal.locator(".xterm-screen").boundingBox();
            if (!box) throw new Error("Terminal screen unavailable");
            // xterm's auxclick handler moves and focuses its textarea, and the engine pastes
            // PRIMARY there on release: headless Chromium from its in-memory selection,
            // WebKitGTK through PasteGlobalSelection.
            await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2, { button: "middle" });
            await frames(page);
            break;
        }
        case "terminalFocus":
            if (step.focused) await terminal.locator(".xterm-screen").click();
            else await terminal.locator("textarea").evaluate((element: HTMLTextAreaElement) => element.blur());
            await frames(page);
            break;
        case "expectTerminalCursor": {
            const cells = await geometry(terminal);
            await page.waitForFunction(({ expected, row, col, left, width }) => {
                const cursor = [...document.querySelectorAll(".xterm .xterm-rows .xterm-cursor")].find((element) => element.getClientRects().length > 0);
                if (!cursor) return false;
                const has = (name: string) => cursor.classList.contains(`xterm-cursor-${name}`);
                const shape = ["outline", "bar", "underline", "block"].find(has);
                const paint = `${shape}${has("blink") ? " blink" : ""}`;
                const line = cursor.closest(".xterm-rows > div");
                const at = Array.prototype.indexOf.call(line?.parentElement?.children ?? [], line);
                const x = Math.round((cursor.getBoundingClientRect().left - left) / width);
                return paint === expected && (row == null || at === row) && (col == null || x === col);
            }, { expected: step.cursor, row: step.row, col: step.col, left: cells.bounds.x, width: cells.width });
            break;
        }
        case "terminalScroll": {
            // xterm 6 scrolls its buffer from wheel input, not from viewport scrollTop.
            const cells = await geometry(terminal);
            await page.mouse.move(cells.bounds.x + cells.bounds.width / 2, cells.bounds.y + cells.bounds.height / 2);
            // One wheel event per row: xterm limits how far one event scrolls.
            for (let row = 0; row < step.rows; row++) await page.mouse.wheel(0, -cells.height);
            await frames(page);
            break;
        }
    }
}
