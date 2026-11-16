import type { Page } from "playwright-core";
import type { Step } from "./contract";
import { resourceRow } from "./web-resource-table";

/**
 * Runs in the page: the Resource metrics card's message and context lines as
 * JSON, or whether they equal `want` when it is not null.
 */
function metricsCard(want: string | null) {
	const card = document.querySelector('[role="group"][aria-label="Resource metrics"]');
	const found = card && JSON.stringify({
		message: card.children[1]?.textContent?.trim() ?? "",
		contexts: Array.from(card.querySelectorAll("li"), (item) => item.textContent?.trim() ?? ""),
	});
	return want === null ? found : found === want;
}

export async function topologyAction(page: Page, step: Step) {
	switch (step.action) {
		case "expectUsageFooter": await page.locator("footer").getByText(step.text, { exact: true }).waitFor(); break;
		case "openUsageTree": await page.getByRole("button", { name: "Show app process tree", exact: true }).click(); break;
		case "expectUsageProcess": await page.getByText(step.label, { exact: true }).waitFor(); break;
		case "expectNoUsageFooter": await page.locator("footer").waitFor({ state: "hidden" }); break;
		case "setTopologyMode":
			await page.getByRole("button", { name: step.mode, exact: true }).click();
			if (!step.pending) await page.getByText("Loading topology", { exact: true }).waitFor({ state: "hidden" });
			break;
		case "expectTopology": await page.getByText(`${step.nodes} nodes / ${step.edges} edges`, { exact: true }).waitFor(); break;
		case "expectTopologyEdge": {
			const edge = page.locator(".svelte-flow__edge").filter({ has: page.locator(`[id="${step.source}->${step.target}"]`) });
			if (!(await edge.count()) && !(await page.locator(`[data-id="${step.source}->${step.target}"]`).count())) throw new Error("Missing traffic edge");
			break;
		}
		case "selectTopologyNode": await page.locator(`.svelte-flow__node[data-id="${step.id}"]`).click(); break;
		case "expectTopologyEmpty": await page.getByText(step.title, { exact: true }).waitFor(); break;
		case "expectNoTopologyNode":
			// The loading spinner shows only after a delay, so its absence does not mean the graph
			// loaded. Wait for a rendered graph, so an empty container cannot pass this check.
			await page.locator(".svelte-flow__node").first().waitFor();
			await page.getByText("Loading topology", { exact: true }).waitFor({ state: "hidden" });
			if (await page.locator(`.svelte-flow__node[data-id="${step.id}"]`).count()) throw new Error("Other mode's node remains");
			break;
		case "expectResourceMetrics": {
			const row = resourceRow(page, step);
			await row.getByLabel(`CPU usage ${step.cpu}`, { exact: true }).waitFor();
			await row.getByLabel(`Memory usage ${step.memory}`, { exact: true }).waitFor();
			break;
		}
		case "expectMetricsCard": {
			const want = JSON.stringify({ message: step.message, contexts: step.contexts });
			// Metrics load after the rows settle, so wait for the card to show the expected state.
			await page.waitForFunction(metricsCard, want).catch(async () => {
				throw new Error(`Resource metrics card showed ${await page.evaluate(metricsCard, null)}`);
			});
			break;
		}
		default:
			throw new Error(`unhandled topology action: ${step.action}`);
	}
}
