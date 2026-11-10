import type { Page } from "playwright-core";
import type { Step } from "./contract";

type Identity = { kind: string; namespace: string; name: string };

/** A live-session popover row, found by the target text it shows. */
function sessionRow(page: Page, target: string) {
	const rows = page.locator('[data-slot="popover-content"][data-state="open"] div.rounded-md.border.bg-background.p-2').filter({ has: page.getByText(target, { exact: true }) });
	return target.includes(":") ? rows : rows.filter({ has: page.getByRole("button", { name: /^Show exec session in / }) });
}

function sessionControl(page: Page, target: string, operation: string) {
	const row = sessionRow(page, target);
	return operation === "Show"
		? row.getByRole("button", { name: /^Show exec session in / })
		: row.getByRole("button", { name: operation, exact: true });
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
			if (await page.getByRole("button", { name: /^Open live sessions \(\d+ active\)$/ }).getAttribute("aria-expanded") !== "true") {
				await page.getByRole("button", { name: /^Open live sessions \(\d+ active\)$/ }).click();
			}
			break;
		case "expectLiveSessionRow": {
			const row = sessionRow(page, step.target);
			await row.getByText(step.status, { exact: true }).waitFor();
			for (const control of step.controls) await sessionControl(page, step.target, control).waitFor();
			break;
		}
		case "liveSessionControl":
			if (step.operation === "Manage") await page.getByRole("button", { name: "Manage", exact: true }).click();
			else await sessionControl(page, step.target ?? "", step.operation).click();
			break;
		case "expectHealthTooltip": {
			const row = resourceRow(page, step);
			const trigger = row.locator("button").filter({ hasText: step.state }).first();
			// Focus survives the lazy trigger replacement through the component's activation handler.
			await trigger.focus();
			const tooltip = page.locator('[data-slot="tooltip-content"]');
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
