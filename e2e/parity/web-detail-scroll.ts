import type { Page } from "playwright-core";

export async function expectDetailScroll(page: Page, view: "events" | "timeline") {
    const selector = view === "events"
        ? 'input[aria-label="Filter resource events"]'
        : 'section[aria-label="Incident timeline"]';
    const anchor = page.locator(selector);
    const geometry = await anchor.evaluate(element => {
        let viewport = element.parentElement;
        while (viewport && !["auto", "scroll"].includes(getComputedStyle(viewport).overflowY)) {
            viewport = viewport.parentElement;
        }
        if (!viewport || viewport.scrollHeight <= viewport.clientHeight) {
            throw new Error("Detail pane has no scrollable overflow");
        }
        const bounds = viewport.getBoundingClientRect();
        return { x: bounds.x + bounds.width / 2, y: bounds.y + bounds.height / 2,
            height: viewport.clientHeight, before: element.getBoundingClientRect().top };
    });
    await page.mouse.move(geometry.x, geometry.y);
    await page.mouse.wheel(0, geometry.height);
    await page.waitForFunction(({ selector, before }) => {
        const element = document.querySelector(selector);
        return element !== null && element.getBoundingClientRect().top < before;
    }, { selector, before: geometry.before });
}

export async function expectResourceEvent(
    page: Page,
    expected: { index: number; reason: string; message: string },
) {
    const row = page.getByText(expected.message, { exact: true }).first()
        .locator("xpath=ancestor::div[time][1]");
    await row.getByText(expected.reason, { exact: true }).waitFor();
    const index = await row.evaluate(element => {
        const siblings = element.parentElement?.children;
        if (!siblings) throw new Error("Event list missing");
        return Array.from(siblings).indexOf(element);
    });
    if (index !== expected.index) {
        throw new Error(`Expected event at ${expected.index}, found ${index}`);
    }
}
