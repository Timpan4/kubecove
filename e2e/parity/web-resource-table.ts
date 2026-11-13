import { isDeepStrictEqual } from "node:util";
import type { Page } from "playwright-core";
import type { Step } from "./contract";

type Identity = { kind: string; name: string; namespace: string };

/** The table rows for one resource, found by its name, kind and namespace, and by its Context cell when `context` is given. */
export function resourceRow(page: Page, step: Identity & { context?: string }) {
	const rows = page.locator("tr").filter({ has: page.getByRole("button", { name: `Open resource ${step.name}`, exact: true }) }).filter({ has: page.getByText(step.kind, { exact: true }) }).filter({ has: page.getByText(step.namespace, { exact: true }) });
	return step.context === undefined ? rows : rows.filter({ has: page.getByRole("cell", { name: step.context, exact: true }) });
}

/**
 * The cell under a column header, located by the header's text in the row's table. Waits for
 * the row first: after a search or a sort the table renders it later, and reading the headers
 * through a missing row finds none.
 */
async function cellFor(page: Page, step: Identity & { column: string }) {
	const row = resourceRow(page, step);
	await row.first().waitFor();
	const headers = (await row.locator("xpath=ancestor::table[1]").locator("thead th").allTextContents()).map((text) => text.trim());
	const index = headers.indexOf(step.column);
	if (index < 0) throw new Error(`No ${step.column} column in the resource table`);
	if (await row.count() !== 1) throw new Error("Resource row is ambiguous");
	return row.getByRole("cell").nth(index);
}

export async function resourceTableAction(page: Page, step: Step) {
	switch (step.action) {
		case "pressResourceRow":
			// The name is a native button: focusing it and pressing Enter or Space clicks it.
			await resourceRow(page, step).getByRole("button", { name: `Open resource ${step.name}`, exact: true }).press(step.key);
			break;
		case "sortResourceTable":
			await page.locator("thead").getByRole("button", { name: `Sort by ${step.column}`, exact: true }).click();
			break;
		case "expectResourceOrder": {
			const visible = async () => {
				const labels = await page.locator('tr button[aria-label^="Open resource "]').evaluateAll((buttons) => buttons.map((button) => button.getAttribute("aria-label") ?? ""));
				return labels.map((label) => label.replace(/^Open resource /, "")).filter((name) => step.names.includes(name));
			};
			// The table re-renders after a sort or a search, so wait for the order.
			await page.waitForFunction((names) => {
				const found = Array.from(document.querySelectorAll('tr button[aria-label^="Open resource "]'), (button) => (button.getAttribute("aria-label") ?? "").replace(/^Open resource /, "")).filter((name) => names.includes(name));
				return JSON.stringify(found) === JSON.stringify(names);
			}, step.names).catch(() => {});
			const order = await visible();
			if (!isDeepStrictEqual(order, step.names)) throw new Error(`Resource order was ${order.join(", ")}`);
			break;
		}
		case "toggleResourceGroup":
			await page.locator("tr").getByRole("button", { name: step.label }).first().click();
			break;
		case "expectResourceGroup":
			await page.locator(`tr button[aria-expanded="${String(step.expanded)}"]`).filter({ hasText: step.label }).first().waitFor();
			break;
		case "openResourceRow":
			await (await cellFor(page, step)).click();
			break;
		case "expectResourceCell": {
			const cell = await cellFor(page, step);
			// A cell can fill after its row renders (metrics, watch updates), so wait for the text.
			const exact = new RegExp(`^\\s*${step.text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}\\s*$`);
			await cell.filter({ hasText: exact }).waitFor().catch(async () => {
				throw new Error(`${step.column} cell was ${(await cell.textContent().catch(() => null))?.trim() ?? "missing"}`);
			});
			break;
		}
		case "expectResourceCellFits": {
			const value = (await cellFor(page, step)).locator("[title], .truncate");
			if (await value.count() !== 1) throw new Error(`${step.column} value was not found in its cell`);
			const fit = await value.evaluate((element) => {
				const cellBox = element.closest("td")?.getBoundingClientRect();
				const box = element.getBoundingClientRect();
				return {
					title: element.getAttribute("title"),
					text: element.textContent?.trim() ?? "",
					inside: cellBox !== undefined && box.left >= cellBox.left - 0.5 && box.right <= cellBox.right + 0.5,
					truncated: element.scrollWidth > element.clientWidth,
					ellipsis: getComputedStyle(element).textOverflow === "ellipsis",
				};
			});
			if (fit.text !== step.text) throw new Error(`${step.column} value was ${fit.text}`);
			if (step.truncated && fit.title !== step.text) throw new Error(`${step.column} hover value was ${fit.title}`);
			if (!fit.inside) throw new Error(`${step.column} value escapes its cell`);
			if (fit.truncated !== step.truncated) throw new Error(`${step.column} value truncated was ${fit.truncated}, expected ${step.truncated}`);
			if (step.truncated && !fit.ellipsis) throw new Error(`${step.column} value is not truncated with an ellipsis`);
			break;
		}
		case "expectResourceColumns": {
			// The resource table is the one with the Name sort header; the pin column's header is screen-reader text.
			const header = page.locator("table").filter({ has: page.getByRole("button", { name: "Sort by Name", exact: true }) }).locator("thead th");
			const labels = async () => (await header.allTextContents()).map((label) => label.trim()).filter((label) => label && label !== "Pin");
			await page.waitForFunction((want) => {
				const table = Array.from(document.querySelectorAll("table")).find((candidate) => candidate.querySelector('button[aria-label="Sort by Name"]'));
				const found = Array.from(table?.querySelectorAll("thead th") ?? [], (head) => head.textContent?.trim() ?? "").filter((label) => label && label !== "Pin");
				return JSON.stringify(found) === want;
			}, JSON.stringify(step.columns)).catch(async () => {
				throw new Error(`Resource table columns were ${(await labels()).join(", ")}`);
			});
			break;
		}
		case "expectResourceActivity":
			await resourceRow(page, step).locator(`canvas[data-resource-progress="${step.state}"]`).waitFor();
			break;
		default:
			throw new Error(`Unhandled resource table action ${step.action}`);
	}
}
