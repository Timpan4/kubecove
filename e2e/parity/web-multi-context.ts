import type { WebActions } from "./web-action";
import type { Page } from "playwright-core";
import { tabTo } from "./web-keyboard";

const escapeRegExp = (value: string) => value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
const mapContextSelect = (page: Page) => page.getByRole("combobox", { name: "Map context", exact: true });
// Bits UI mounts a select's listbox only while it is open, so on the map this is the Map context options.
const mapContextOptions = (page: Page) => page.getByRole("listbox");

/**
 * Runs in the page: the Context cells of every table row with this identity, in
 * display order, or whether they equal `want` when it is given.
 */
function rowContexts({ kind, name, namespace, want }: { kind: string; name: string; namespace: string; want?: string }) {
	const table = Array.from(document.querySelectorAll("table")).find((candidate) => candidate.querySelector('button[aria-label="Sort by Name"]'));
	if (!table) return want === undefined ? null : false;
	const headers = Array.from(table.querySelectorAll("thead th"), (head) => head.textContent?.trim() ?? "");
	const [context, kindColumn, namespaceColumn] = ["Context", "Kind", "Namespace"].map((label) => headers.indexOf(label));
	if (context < 0) return want === undefined ? null : false;
	const contexts: string[] = [];
	for (const row of table.querySelectorAll("tbody tr")) {
		const cells = Array.from(row.querySelectorAll("td"), (cell) => cell.textContent?.trim() ?? "");
		if (!row.querySelector(`button[aria-label="Open resource ${CSS.escape(name)}"]`)) continue;
		if (cells[kindColumn] === kind && cells[namespaceColumn] === namespace) contexts.push(cells[context]);
	}
	return want === undefined ? contexts : JSON.stringify(contexts) === want;
}

export const multiContextActions = {
	async expectRowContexts(page, step) {
		const identity = { kind: step.kind, name: step.name, namespace: step.namespace };
		await page.waitForFunction(rowContexts, { ...identity, want: JSON.stringify(step.contexts) }).catch(async () => {
			const found = await page.evaluate(rowContexts, identity);
			throw new Error(`${step.kind} ${step.namespace}/${step.name} rows had contexts ${JSON.stringify(found)}`);
		});
	},
	async filterContexts(page, step) {
		await page.getByRole("button", { name: "Edit context filter", exact: true }).click();
		const options = page.locator('[data-slot="popover-content"]').filter({ has: page.getByText("Contexts", { exact: true }) }).locator("label");
		await options.first().waitFor();
		for (let index = 0; index < await options.count(); index += 1) {
			const option = options.nth(index);
			const want = step.contexts.includes((await option.textContent())?.trim() ?? "");
			const checkbox = option.getByRole("checkbox");
			if ((await checkbox.getAttribute("aria-checked") === "true") !== want) await checkbox.click();
		}
		await page.keyboard.press("Escape");
	},
	async expectResourceSummary(page, step) {
		await page.getByText(step.text, { exact: true }).waitFor();
	},
	async selectMapContext(page, step) {
		await mapContextSelect(page).click();
		await page.getByRole("option", { name: step.context, exact: true }).click();
	},
	async mapContextKey(page, step) {
		// While the options are open, keys go to the select wherever Bits UI keeps focus.
		if (!(await mapContextOptions(page).isVisible())) await tabTo(page, mapContextSelect(page));
		await page.keyboard.press(step.key);
	},
	async expectMapContextOptions(page, step) {
		await mapContextOptions(page).waitFor({ state: step.open ? "visible" : "hidden" });
		if (step.focused !== undefined) {
			const focused = await mapContextSelect(page).evaluate((node) => node === document.activeElement);
			if (focused !== step.focused) throw new Error(`Map context trigger focused: ${focused}, expected ${step.focused}`);
		}
	},
	async expectMapContext(page, step) {
		// The map header is present, so a missing select is not a map that has not loaded yet.
		await page.getByRole("button", { name: "Collapse ownership map", exact: true }).waitFor();
		if (!step.shown) {
			if (await mapContextSelect(page).count()) throw new Error("The ownership map shows a Map context select");
			return;
		}
		await mapContextSelect(page).filter({ hasText: new RegExp(`^\\s*${escapeRegExp(step.context)}\\s*$`) }).waitFor();
	},
} satisfies Partial<WebActions>;
