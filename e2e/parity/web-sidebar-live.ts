import type { Locator, Page } from "playwright-core";
import type { Step } from "./contract";

/** The sidebar's scroll area, which wraps the Kubernetes resource tree. */
function sidebarScrollArea(page: Page): Locator {
	return page.getByRole("navigation", { name: "Kubernetes resource tree", exact: true })
		.locator("xpath=ancestor::div[@data-slot='scroll-area'][1]");
}

/** The first row whose label matches each path segment, at that depth. */
function sidebarRow(page: Page, path: string[]): Locator {
	let list = page.getByRole("tree");
	for (const [index, label] of path.entries()) {
		const row = list.getByRole("treeitem").and(page.locator(`[data-depth="${index}"]`)).filter({ has: page.getByText(label, { exact: true }) }).first();
		if (index === path.length - 1) return row;
		list = row.locator("..").getByRole("group").first();
	}
	throw new Error("Empty sidebar path");
}

export async function sidebarLiveAction(page: Page, step: Step) {
    switch (step.action) {
        case "sidebarToggle": case "sidebarSelect": case "expectSidebarRow": {
            const row = sidebarRow(page, step.path);
            if (step.action === "sidebarToggle") await row.getByRole("button").click();
            else if (step.action === "sidebarSelect") await row.click();
            else if (step.action === "expectSidebarRow") {
                await row.waitFor({ state: step.visible ? "visible" : "hidden" });
                if (step.visible && (await row.getAttribute("aria-selected") === "true") !== step.selected) throw new Error(`Wrong sidebar selection: ${step.path.join("/")}`);
            }
            break;
        }
        case "scrollSidebar": await sidebarScrollArea(page).evaluate((element, to) => { element.scrollTop = to === "bottom" ? element.scrollHeight : 0; }, step.to); break;
        case "expectSidebarReveal": {
            const row = sidebarRow(page, step.path);
            if (step.visible || step.highlighted) await row.waitFor({ state: "attached" });
            const rowHandle = (await row.count()) > 0 ? await row.elementHandle() : null;
            const scrollHandle = await sidebarScrollArea(page).elementHandle();
            // The reveal scrolls on the frame after the row paints, so poll the geometry and the label marker.
            const state = { row: rowHandle, scroll: scrollHandle, visible: step.visible, highlighted: step.highlighted };
            await page.waitForFunction((check: { row: Element | null; scroll: Element | null; visible: boolean; highlighted: boolean }) => {
                if (!check.scroll) return false;
                const viewport = check.scroll.getBoundingClientRect();
                const rowVisible = check.row !== null && (() => {
                    const box = check.row.getBoundingClientRect();
                    return box.top >= viewport.top && box.bottom <= viewport.bottom;
                })();
                const marker = check.row?.querySelector("[aria-description]")?.getAttribute("aria-description");
                return rowVisible === check.visible && (marker === "Contains the selected item") === check.highlighted;
            }, state);
            break;
        }
        case "collapseSidebar": await page.getByRole("button", { name: "Collapse all sidebar sections", exact: true }).click(); break;
        case "expectSidebarBranch": {
            const label = `${step.expanded ? "Collapse" : "Expand"} ${step.page}`;
            await page.getByRole("button", { name: label, exact: true }).waitFor();
            break;
        }
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
