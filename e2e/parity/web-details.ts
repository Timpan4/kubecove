import type { Page } from "playwright-core";
import type { Step } from "./contract";

type DetailsStep = Extract<Step, { action: "clickDetailControl" | "expectNoPageText" }>;

/** Details metadata controls are matched by visible text, the same text the native driver labels. */
export async function detailsAction(page: Page, step: DetailsStep) {
	switch (step.action) {
		case "clickDetailControl":
			await page.getByText(step.label, { exact: true }).first().click();
			break;
		case "expectNoPageText":
			if (await page.getByText(step.text, { exact: true }).count()) throw new Error(`${step.text} is shown`);
			break;
	}
}
