import type { Page } from "playwright-core";
import type { Step } from "./contract";

const DETAIL_SEPARATOR = { name: "Resize details panel", exact: true } as const;

/** Map and table collapse, graph focus, and the detail splitter (`ResourceBrowser.svelte`, `OwnershipMap.svelte`, `DetailPanelFrame.svelte`). */
export async function panelAction(page: Page, step: Step): Promise<void> {
	switch (step.action) {
		case "panelControl":
			await page.getByRole("button", { name: step.control, exact: true }).first().click();
			break;
		case "expectPanelControl": {
			const control = page.getByRole("button", { name: step.control, exact: true }).first();
			await control.waitFor({ state: step.present ? "attached" : "detached" });
			break;
		}
		case "skipGraph":
			await page.getByRole("button", { name: "skip graph", exact: true }).click();
			break;
		case "mapKey":
			await page.keyboard.press(step.key);
			break;
		case "expectMapFocus":
		case "expectNoMapFocus": {
			const focused = await page.locator(`.svelte-flow__node[data-id="${step.id}"]`).evaluate((node) => node === document.activeElement);
			if (focused !== (step.action === "expectMapFocus")) throw new Error(`Graph node focus differs for ${step.id}`);
			break;
		}
		case "resizeDetail":
			await page.getByRole("separator", DETAIL_SEPARATOR).focus();
			await page.keyboard.press(step.key);
			break;
		case "expectDetailResize": {
			const value = await page.getByRole("separator", DETAIL_SEPARATOR).getAttribute("aria-valuenow");
			if (value !== String(step.value)) throw new Error(`Detail splitter is ${value}, expected ${step.value}`);
			break;
		}
		default:
			throw new Error(`Unhandled panel action: ${step.action}`);
	}
}
