import type { Locator, Page } from "playwright-core";

const node = ".svelte-flow__node-ownershipResource";
/** The highlight a selected resource card shows, from OwnershipResourceNode.svelte's `data-selected`. */
const selectedNode = `${node} [data-selected="true"]`;
/** The map animates its initial fit for this long (OwnershipMapViewport.svelte, FIT_VIEW_DURATION_MS). */
const FIT_VIEW_DURATION_MS = 260;
/** Pixels of layout rounding allowed when comparing measured node boxes. */
const LAYOUT_ROUNDING_PX = 1;
/** One wheel notch of 100 px, which xyflow turns into a zoom step of 2 ** (100 * 0.002). */
const WHEEL_PIXELS = 100;
const WHEEL_STEP = 2 ** (WHEEL_PIXELS * 0.002);

declare global {
	interface Window { mapSettle?: { style: string; since: number } }
}

type Point = { x: number; y: number };

/** Map pointer gestures: a drag that starts on a node, and a wheel that zooms about the cursor. */
export async function checkMapGesture(page: Page, check: string) {
	await page.locator(".svelte-flow").scrollIntoViewIfNeeded();
	await page.locator(node).first().waitFor();
	await waitForFitToSettle(page);
	const first = page.locator(node).first();
	const center = await centerOf(first);
	if (check === "map-drag-node") {
		await dragFromNode(page, center);
		return;
	}
	await wheelAtNode(page, center);
}

/**
 * Resolves once the viewport transform has not changed for FIT_VIEW_DURATION_MS. Input during the
 * fit animation would be overwritten by it, so the gesture waits for the map to settle first.
 */
async function waitForFitToSettle(page: Page) {
	await page.waitForFunction(
		(duration) => {
			const style = document.querySelector<HTMLElement>(".svelte-flow__viewport")?.getAttribute("style") ?? "";
			const now = performance.now();
			window.mapSettle ??= { style, since: now };
			if (window.mapSettle.style !== style) window.mapSettle = { style, since: now };
			return now - window.mapSettle.since >= duration;
		},
		FIT_VIEW_DURATION_MS,
		{ polling: "raf" },
	);
}

async function centerOf(locator: Locator): Promise<Point> {
	const box = await locator.boundingBox();
	if (!box) throw new Error("Map node is missing");
	return { x: box.x + box.width / 2, y: box.y + box.height / 2 };
}

/** The id of the node whose card shows the selected highlight, or undefined when none does. */
function selectedId(page: Page) {
	return page.evaluate((selector) => document.querySelector(selector)?.closest(".svelte-flow__node")?.getAttribute("data-id"), selectedNode);
}

async function dragFromNode(page: Page, center: Point) {
	// A press and release without movement still selects. Select the first node that way, then drag
	// from another node: the drag must pan and must leave that selection alone.
	await page.mouse.click(center.x, center.y);
	await page.locator(selectedNode).first().waitFor();
	const selected = await selectedId(page);
	await waitForFitToSettle(page);
	const target = await centerOf(page.locator(node).nth(1));
	const viewport = page.locator(".svelte-flow__viewport");
	const before = await viewport.getAttribute("style");
	await page.mouse.move(target.x, target.y);
	await page.mouse.down();
	await page.mouse.move(target.x + 80, target.y + 60, { steps: 8 });
	await page.mouse.up();
	if (before === (await viewport.getAttribute("style"))) throw new Error("Drag from a node did not pan the map");
	if ((await selectedId(page)) !== selected) throw new Error("Dragging from a node changed the selection");
}

async function wheelAtNode(page: Page, center: Point) {
	const before = await page.locator(node).first().boundingBox();
	if (!before) throw new Error("Map node is missing");
	// The node under the cursor keeps its center under the cursor, so the zoom is anchored at the pointer.
	await page.mouse.move(center.x, center.y);
	await page.mouse.wheel(0, -WHEEL_PIXELS);
	await page.waitForFunction(
		({ selector, previous }) =>
			(document.querySelector(selector)?.getBoundingClientRect().width ?? 0) > previous,
		{ selector: node, previous: before.width },
	);
	const after = await page.locator(node).first().boundingBox();
	if (!after) throw new Error("Map node is missing after the wheel");
	if (Math.abs(after.width - before.width * WHEEL_STEP) > LAYOUT_ROUNDING_PX) {
		throw new Error(`Wheel zoom did not apply one wheel step: width ${before.width} became ${after.width}`);
	}
	const afterCenter = { x: after.x + after.width / 2, y: after.y + after.height / 2 };
	if (Math.abs(afterCenter.x - center.x) > LAYOUT_ROUNDING_PX || Math.abs(afterCenter.y - center.y) > LAYOUT_ROUNDING_PX) {
		throw new Error("Wheel zoom was not anchored at the cursor");
	}
}
