import type { Page } from "playwright-core";
import type { Step } from "./contract";

type DetailsStep = Extract<Step, { action: "clickDetailControl" }>;

/** Details metadata controls are matched by visible text, the same text the native driver labels. */
export async function detailsAction(page: Page, step: DetailsStep) {
	switch (step.action) {
		case "clickDetailControl":
			await page.getByText(step.label, { exact: true }).first().click();
			break;
	}
}
