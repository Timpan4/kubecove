import type { Page } from "playwright-core";
import type { OverviewStep } from "./contract-overview";
const literal = (value: string) => value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
export async function overviewAction(page: Page, step: OverviewStep): Promise<void> {
	switch (step.action) {
		case "overviewHealth":
			await page.getByRole("button", { name: new RegExp(`^${literal(step.filter)} \\d+$`) }).click();
			break;
		case "overviewNamespace":
			await page.getByRole("button", { name: `Open namespace ${step.namespace}`, exact: true }).click();
			break;
		case "expectOverviewProblem": case "openOverviewProblem": case "toggleOverviewProblem": case "expectNoOverviewProblem": {
			const suffix = `:${step.kind}:${step.namespace}:${step.name}`;
			const row = page.getByRole("table", { name: "Workspace findings", exact: true })
				.locator(`[data-overview-resource^=${JSON.stringify(`${step.context}:`)}][data-overview-resource$=${JSON.stringify(suffix)}]`);
			if (step.action === "expectNoOverviewProblem") { await row.waitFor({ state: "hidden" }); break; }
			await row.waitFor();
			if (step.action === "openOverviewProblem") await row.locator("td").first().getByRole("button").first().click();
			else if (step.action === "toggleOverviewProblem") await row.getByRole("button", { name: /^Owned resources of / }).click();
			else {
				await row.locator("td").nth(1).getByText(step.status, { exact: true }).waitFor();
				await row.locator("td").nth(2).getByText(step.ready, { exact: true }).waitFor();
			}
			break;
		}
	}
}
