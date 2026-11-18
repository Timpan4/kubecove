import { sharedActions, type WebActions } from "./web-action";
import type { Locator, Page } from "playwright-core";

/** The sidebar's scroll area, which wraps the Kubernetes resource tree. */
function sidebarScrollArea(page: Page): Locator {
	return page.getByRole("navigation", { name: "Kubernetes resource tree", exact: true })
		.locator("xpath=ancestor::div[@data-slot='scroll-area'][1]");
}

/** The first row whose label matches each path segment, at that depth. */
export function sidebarRow(page: Page, path: string[]): Locator {
	let list = page.getByRole("tree");
	for (const [index, label] of path.entries()) {
		const row = list.getByRole("treeitem").and(page.locator(`[data-depth="${index}"]`)).filter({ has: page.getByText(label, { exact: true }) }).first();
		if (index === path.length - 1) return row;
		list = row.locator("..").getByRole("group").first();
	}
	throw new Error("Empty sidebar path");
}

export const sidebarLiveActions = {
	...sharedActions(["sidebarToggle", "sidebarSelect", "expectSidebarRow"], async (page, step) => {
		const row = sidebarRow(page, step.path);
		if (step.action === "sidebarToggle") await row.getByRole("button").click();
		else if (step.action === "sidebarSelect") await row.click();
		else if (step.action === "expectSidebarRow") {
			await row.waitFor({ state: step.visible ? "visible" : "hidden" });
			if (step.visible && (await row.getAttribute("aria-selected") === "true") !== step.selected) throw new Error(`Wrong sidebar selection: ${step.path.join("/")}`);
		}
	}),
	async scrollSidebar(page, step) {
		await sidebarScrollArea(page).evaluate((element, to) => { element.scrollTop = to === "bottom" ? element.scrollHeight : 0; }, step.to);
	},
	async expectSidebarReveal(page, step) {
		const row = sidebarRow(page, step.path);
		if (step.visible || step.highlighted) await row.waitFor({ state: "attached" });
		const rowHandle = (await row.count()) > 0 ? await row.elementHandle() : null;
		const scrollHandle = await sidebarScrollArea(page).elementHandle();
		// The reveal scrolls on the frame after the row paints, so poll the geometry and the row's hint.
		const state = { row: rowHandle, scroll: scrollHandle, visible: step.visible, highlighted: step.highlighted };
		await page.waitForFunction((check: { row: Element | null; scroll: Element | null; visible: boolean; highlighted: boolean }) => {
			if (!check.scroll) return false;
			const viewport = check.scroll.getBoundingClientRect();
			const rowVisible = check.row !== null && (() => {
				const box = check.row.getBoundingClientRect();
				return box.top >= viewport.top && box.bottom <= viewport.bottom;
			})();
			// The treeitem carries the hint itself, so it is announced when the row has focus.
			const marker = check.row?.getAttribute("aria-description");
			return rowVisible === check.visible && (marker === "Contains the selected item") === check.highlighted;
		}, state);
	},
	async collapseSidebar(page) {
		await page.getByRole("button", { name: "Collapse all sidebar sections", exact: true }).click();
	},
	async expectSidebarBranch(page, step) {
		const label = `${step.expanded ? "Collapse" : "Expand"} ${step.page}`;
		await page.getByRole("button", { name: label, exact: true }).waitFor();
	},
	async refreshPage(page) {
		await page.getByRole("button", { name: "Refresh", exact: true }).click();
	},
	async replayPageReads(page, step) {
		await page.evaluate(`import('/src/lib/tauri-dev-mock-pages.ts').then(m => m.replayPageReads(${JSON.stringify(step.phase)}))`);
	},
	async focusStalePage(page, step) {
		await page.clock.install();
		await page.evaluate("import('/node_modules/.vite/deps/@tanstack_svelte-query.js').then(m => m.focusManager.setFocused(false))");
		// Query staleTime from the named Svelte surface. No polling is added.
		await page.clock.fastForward(step.page === "RBAC" ? 30_000 : 15_000);
		await page.evaluate("import('/node_modules/.vite/deps/@tanstack_svelte-query.js').then(m => m.focusManager.setFocused(true))");
	},
	async expectPageText(page, step) {
		// Open the disclosure only while it is closed, so repeated checks do not toggle it shut.
		if (step.text === "Scripted page refresh failure" && !(await page.getByText(step.text, { exact: true }).first().isVisible())) await page.getByText("Technical detail", { exact: true }).first().click();
		await page.getByText(step.text, { exact: true }).first().waitFor();
	},
	async searchCustomKinds(page, step) {
		const input = page.getByRole("textbox", { name: "Search custom resources", exact: true });
		if (!(await input.isVisible())) await page.getByRole("button", { name: "Expand Custom Resources", exact: true }).click();
		await input.fill(step.value);
	},
	async expectCustomGroup(page, step) {
		await page.getByRole("button", { name: `Expand ${step.group}`, exact: true }).waitFor({ state: step.visible ? "visible" : "hidden" });
	},
	async openKindPicker(page) {
		await page.getByRole("button", { name: "Edit resource kinds", exact: true }).click();
	},
	async closeKindPicker(page) {
		await page.keyboard.press("Escape");
	},
	async searchKindPicker(page, step) {
		await page.getByRole("textbox", { name: "Search resource kinds", exact: true }).fill(step.value);
	},
	async selectAllKinds(page) {
		await page.getByRole("button", { name: "Select all kinds", exact: true }).click();
	},
	...sharedActions(["toggleKind", "expectKindChecked"], async (page, step) => {
		const option = page.locator("label").filter({ has: page.getByText(step.kind, { exact: true }) }).getByRole("checkbox");
		await option.waitFor();
		const checked = await option.getAttribute("aria-checked") === "true";
		if (step.action === "toggleKind") { if (checked !== step.checked) await option.click(); }
		else if (checked !== step.checked) throw new Error(`${step.kind} checked=${checked}, expected ${step.checked}`);
	}),
} satisfies Partial<WebActions>;
