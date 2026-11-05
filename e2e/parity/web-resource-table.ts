import { isDeepStrictEqual } from "node:util";
import type { Page } from "playwright-core";
import type { Step } from "./contract";

type Identity = { kind: string; name: string; namespace: string };

/** The table row for one resource, found by its name, kind and namespace. */
export function resourceRow(page: Page, step: Identity) {
	return page.locator("tr").filter({ has: page.getByRole("button", { name: `Open resource ${step.name}`, exact: true }) }).filter({ has: page.getByText(step.kind, { exact: true }) }).filter({ has: page.getByText(step.namespace, { exact: true }) });
}

/** The cell under a column header, located by the header's text in the row's table. */
async function cellFor(page: Page, step: Identity & { column: string }) {
	const row = resourceRow(page, step);
	const headers = (await row.locator("xpath=ancestor::table[1]").locator("thead th").allTextContents()).map((text) => text.trim());
	const index = headers.indexOf(step.column);
	if (index < 0) throw new Error(`No ${step.column} column in the resource table`);
	if (await row.count() !== 1) throw new Error("Resource row is ambiguous");
	return row.getByRole("cell").nth(index);
}

export async function resourceTableAction(page: Page, step: Step) {
	switch (step.action) {
		case "sortResourceTable":
			await page.locator("thead").getByRole("button", { name: `Sort by ${step.column}`, exact: true }).click();
			break;
		case "expectResourceOrder": {
			const labels = await page.locator('tr button[aria-label^="Open resource "]').evaluateAll((buttons) => buttons.map((button) => button.getAttribute("aria-label") ?? ""));
			const visible = labels.map((label) => label.replace(/^Open resource /, "")).filter((name) => step.names.includes(name));
			if (!isDeepStrictEqual(visible, step.names)) throw new Error(`Resource order was ${visible.join(", ")}`);
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
			const actual = (await (await cellFor(page, step)).textContent())?.trim();
			if (actual !== step.text) throw new Error(`${step.column} cell was ${actual ?? "missing"}`);
			break;
		}
		default:
			throw new Error(`Unhandled resource table action ${step.action}`);
	}
}
