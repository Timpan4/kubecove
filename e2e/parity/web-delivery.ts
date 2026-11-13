import type { Page } from "playwright-core";
import type { Step } from "./contract";
import { revealGitOps } from "./web-delivery-helpers";

function literal(text: string) { return text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"); }

/** The group headers of the GitOps list, in list order. */
function gitOpsGroups(page: Page) { return page.locator("[data-gitops-group]"); }

/** GitOps list steps. */
export async function deliveryAction(page: Page, step: Step): Promise<void> {
	switch (step.action) {
		case "expectGitOpsGroup": {
			const group = gitOpsGroups(page)
				.and(page.getByRole("button", { name: `${step.label} ${step.count}`, exact: true }))
				.and(page.locator(`[data-gitops-group-position="${step.position}"][aria-expanded="${String(step.expanded)}"]`));
			await revealGitOps(page, group);
			await group.waitFor();
			break;
		}
		// "Healthy" is part of "Not healthy", so the label must start the header's name.
		case "toggleGitOpsGroup": {
			const group = gitOpsGroups(page).and(page.getByRole("button", { name: new RegExp(`^${literal(step.label)} \\d+$`) }));
			await revealGitOps(page, group);
			await group.click();
			break;
		}
		default: throw new Error(`Not a GitOps list step: ${step.action}`);
	}
}
