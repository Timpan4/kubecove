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
			if (focused !== (step.action === "expectMapFocus")) {
				const active = await page.evaluate(() => document.activeElement?.closest(".svelte-flow__node")?.getAttribute("data-id") ?? document.activeElement?.getAttribute("aria-label") ?? document.activeElement?.tagName ?? "none");
				throw new Error(`Graph node focus differs for ${step.id}; focused: ${active}`);
			}
			break;
		}
		case "panMap": {
			// Drag the pane from its center, as a pointer pan does (`panOnDrag`).
			const flow = page.locator(".svelte-flow.ownership-map-flow");
			const box = await flow.boundingBox();
			if (!box) throw new Error("Ownership map has no box to pan");
			const from = { x: box.x + box.width / 2, y: box.y + box.height / 2 };
			await page.mouse.move(from.x, from.y);
			await page.mouse.down();
			await page.mouse.move(from.x + step.dx, from.y + step.dy, { steps: 10 });
			await page.mouse.up();
			break;
		}
		case "expectMapNodeInView": {
			// xyflow's viewport is the `.svelte-flow` container. A node is in view when its box overlaps that box.
			const inView = await page.locator(`.svelte-flow__node[data-id="${step.id}"]`).evaluate((node) => {
				const flow = node.closest(".svelte-flow");
				if (!flow) throw new Error("Graph node has no flow container");
				const a = node.getBoundingClientRect();
				const b = flow.getBoundingClientRect();
				return a.left < b.right && a.right > b.left && a.top < b.bottom && a.bottom > b.top;
			});
			if (inView !== step.inView) throw new Error(`Graph node ${step.id} in view is ${inView}, expected ${step.inView}`);
			break;
		}
		case "clickMapControl":
			await page.getByRole("button", { name: step.control, exact: true }).click();
			break;
		case "expectMapControlFocus": {
			const focused = await page.getByRole("button", { name: step.control, exact: true }).evaluate((node) => node === document.activeElement);
			if (!focused) throw new Error(`${step.control} is not focused`);
			break;
		}
		case "expectMapZoom": {
			const scale = await page.locator(".svelte-flow__viewport").evaluate((node) => new DOMMatrixReadOnly(getComputedStyle(node).transform).a);
			if (scale.toFixed(2) !== step.zoom.toFixed(2)) throw new Error(`Map zoom is ${scale.toFixed(2)}, expected ${step.zoom.toFixed(2)}`);
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
