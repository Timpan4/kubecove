import type { Page } from "playwright-core";
import type { Step } from "./contract";

type Identity = { kind: string; namespace: string; name: string };

/** A live-session popover row, found by the target text it shows. */
function sessionRow(page: Page, target: string) {
	return page.locator("div.rounded-md.border.bg-background.p-2").filter({ hasText: target });
}

function resourceRow(page: Page, step: Identity) {
	return page.locator("tr").filter({ has: page.getByRole("button", { name: `Open resource ${step.name}`, exact: true }) }).filter({ has: page.getByText(step.kind, { exact: true }) }).filter({ has: page.getByText(step.namespace, { exact: true }) });
}

function eventRows(page: Page) {
	return page.locator("div.grid:has(> time[datetime])");
}

/** `formatExactTimestamp` at millisecond precision, with the UTC suffix when the preference is UTC. */
const exactTimestamp = /^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}\.\d{3}( UTC)?$/;

export async function sessionHealthAction(page: Page, step: Step): Promise<void> {
	switch (step.action) {
		case "openLiveSessionPopover":
			await page.getByRole("button", { name: /^Open live sessions \(\d+ active\)$/ }).click();
			break;
		case "expectLiveSessionRow": {
			const row = sessionRow(page, step.target);
			await row.getByText(step.status, { exact: true }).waitFor();
			for (const control of step.controls) await row.getByRole("button", { name: control, exact: true }).waitFor();
			break;
		}
		case "liveSessionControl":
			if (step.operation === "Manage") await page.getByRole("button", { name: "Manage", exact: true }).click();
			else await sessionRow(page, step.target ?? "").getByRole("button", { name: step.operation, exact: true }).click();
			break;
		case "expectHealthTooltip": {
			const row = resourceRow(page, step);
			const trigger = row.locator("button").filter({ hasText: step.state }).first();
			// The first hover swaps the lazy trigger for the Tooltip component, so hover again to open it.
			await trigger.hover();
			await trigger.hover();
			const tooltip = page.getByRole("tooltip");
			await tooltip.getByText(step.state, { exact: true }).waitFor();
			await tooltip.getByText(step.source, { exact: true }).waitFor();
			for (const status of step.statuses) await tooltip.getByText(status, { exact: true }).waitFor();
			const partial = row.getByText("Partial", { exact: true });
			if (step.partial) await partial.first().waitFor();
			else if ((await partial.count()) > 0) throw new Error("Complete assessment shows a Partial marker");
			break;
		}
		case "expectTimestampTooltip": {
			const row = resourceRow(page, step);
			await row.locator("time").first().waitFor();
			const title = await row.locator("time").first().getAttribute("title");
			if (title !== step.value) throw new Error(`Timestamp tooltip was ${title ?? "missing"}`);
			break;
		}
		case "expectEventTimeTooltip": {
			const row = eventRows(page).nth(step.index);
			await row.waitFor();
			const title = await row.locator("time").first().getAttribute("title");
			if (!title || !exactTimestamp.test(title)) throw new Error(`Event time tooltip was ${title ?? "missing"}`);
			break;
		}
	}
}
