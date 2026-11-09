import type { Page } from "playwright-core";
import type { Step } from "./contract";

type MultiContextStep = Extract<Step, { action: "expectRowContexts" | "filterContexts" | "expectResourceSummary" | "selectMapContext" | "expectMapContext" }>;

// TODO(multi-context S5): replace with Playwright handlers once the web client renders the Context column, filter, status line and map picker.
export async function multiContextAction(_page: Page, step: MultiContextStep): Promise<void> {
	throw new Error(`The web driver has no handler for ${step.action} yet`);
}
