import type { Page } from "playwright-core";
import type { Step } from "./contract";

function literal(text: string) { return text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"); }

/** The group headers of the GitOps list, in list order. */
function gitOpsGroups(page: Page) { return page.locator("[data-gitops-group]"); }

/** GitOps list steps. */
export async function deliveryAction(page: Page, step: Step): Promise<void> {
	switch (step.action) {
		case "expectGitOpsGroup":
			await gitOpsGroups(page).nth(step.position)
				.and(page.getByRole("button", { name: `${step.label} ${step.count}`, exact: true }))
				.and(page.locator(`[aria-expanded="${String(step.expanded)}"]`))
				.waitFor();
			break;
		// "Healthy" is part of "Not healthy", so the label must start the header's name.
		case "toggleGitOpsGroup":
			await gitOpsGroups(page).and(page.getByRole("button", { name: new RegExp(`^${literal(step.label)} \\d+$`) })).click();
			break;
		default: throw new Error(`Not a GitOps list step: ${step.action}`);
	}
}
