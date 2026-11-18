import { escapeRegExp } from "./web-locators";
import { sharedActions, type WebActions } from "./web-action";
import { resourceKey } from "../../src/lib/resource-identity";
export const overviewActions = {
	async overviewHealth(page, step) {
		await page.getByRole("button", { name: new RegExp(`^${escapeRegExp(step.filter)} \\d+$`) }).click();
	},
	async overviewNamespace(page, step) {
		await page.getByRole("button", { name: `Open namespace ${step.namespace}`, exact: true }).click();
	},
	...sharedActions(["expectOverviewProblem", "openOverviewProblem", "toggleOverviewProblem", "expectNoOverviewProblem"], async (page, step) => {
		const suffix = `:${step.kind}:${step.namespace}:${step.name}`;
		const selector = step.apiVersion === undefined
			? `[data-overview-resource^=${JSON.stringify(`${step.context}:`)}][data-overview-resource$=${JSON.stringify(suffix)}]`
			: `[data-overview-resource=${JSON.stringify(resourceKey({ cluster: step.context, apiVersion: step.apiVersion, kind: step.kind, namespace: step.namespace, name: step.name }))}]`;
		const row = page.getByRole("table", { name: "Workspace findings", exact: true })
			.locator(selector);
		if (step.action === "expectNoOverviewProblem") { await row.waitFor({ state: "hidden" }); return; }
		await row.waitFor();
		if (step.action === "openOverviewProblem" || step.action === "toggleOverviewProblem") {
			const button = step.action === "openOverviewProblem" ? row.locator("td").first().getByRole("button").first() : row.getByRole("button", { name: /^Owned resources of / });
			if (step.key) { await button.focus(); await button.press(step.key === "enter" ? "Enter" : "Space"); }
			else await button.click();
		}
		else {
			await row.locator("td").nth(1).getByText(step.status, { exact: true }).waitFor();
			await row.locator("td").nth(2).getByText(step.ready, { exact: true }).waitFor();
		}
	}),
} satisfies Partial<WebActions>;
