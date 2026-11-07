import type { Page } from "playwright-core";

type Box = { x: number; y: number; width: number; height: number };

function disjoint(a: Box, b: Box): boolean {
	return a.x + a.width <= b.x || b.x + b.width <= a.x || a.y + a.height <= b.y || b.y + b.height <= a.y;
}

function inside(inner: Box, outer: Box): boolean {
	return inner.x >= outer.x && inner.x + inner.width <= outer.x + outer.width;
}

// The status text may shrink, but its box must not overlap the container picker or the toolbar
// buttons, and every button stays inside the detail panel.
export async function expectLogToolbarFit(page: Page, state: "Live" | "Paused"): Promise<void> {
	const tabpanel = page.getByRole("tabpanel");
	const status = tabpanel.locator("span.whitespace-nowrap").filter({ hasText: state }).filter({ hasText: "lines" });
	const controls = {
		container: page.getByRole("combobox", { name: "Log container", exact: true }),
		tail: page.getByRole("button", { name: "Log tail and time window", exact: true }),
		pause: page.getByRole("button", { name: state === "Paused" ? "Resume" : "Pause", exact: true }),
	};
	// The detail panel is the inspector section around the tab panel.
	const panel = tabpanel.locator("xpath=ancestor::section[1]");
	const boxes: Record<string, Box> = {};
	for (const [name, locator] of Object.entries({ panel, status, ...controls })) {
		await locator.waitFor();
		const box = await locator.boundingBox();
		if (!box) throw new Error(`Log toolbar ${name} has no box`);
		boxes[name] = box;
	}
	for (const name of Object.keys(controls)) {
		if (!disjoint(boxes.status, boxes[name])) throw new Error(`Log status overlaps ${name}: ${JSON.stringify(boxes)}`);
		if (!inside(boxes[name], boxes.panel)) throw new Error(`Log ${name} leaves the detail panel: ${JSON.stringify(boxes)}`);
	}
}
