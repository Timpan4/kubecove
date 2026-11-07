import type { Page } from "playwright-core";

type Box = { x: number; y: number; width: number; height: number };

function disjoint(a: Box, b: Box): boolean {
	return a.x + a.width <= b.x || b.x + b.width <= a.x || a.y + a.height <= b.y || b.y + b.height <= a.y;
}

// The status text may shrink, but its box must not overlap the container picker or the toolbar buttons.
export async function expectLogToolbarFit(page: Page, state: "Live" | "Paused"): Promise<void> {
	const status = page.getByRole("tabpanel").locator("span.whitespace-nowrap").filter({ hasText: state }).filter({ hasText: "lines" });
	const controls = {
		container: page.getByRole("combobox", { name: "Log container", exact: true }),
		tail: page.getByRole("button", { name: "Log tail and time window", exact: true }),
		pause: page.getByRole("button", { name: state === "Paused" ? "Resume" : "Pause", exact: true }),
	};
	const boxes: Record<string, Box> = {};
	for (const [name, locator] of Object.entries({ status, ...controls })) {
		await locator.waitFor();
		const box = await locator.boundingBox();
		if (!box) throw new Error(`Log toolbar ${name} has no box`);
		boxes[name] = box;
	}
	for (const name of Object.keys(controls)) {
		if (!disjoint(boxes.status, boxes[name])) throw new Error(`Log status overlaps ${name}: ${JSON.stringify(boxes)}`);
	}
}
