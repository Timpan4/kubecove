import type { Page } from "playwright-core";
import type { WebActions } from "./web-action";
import { clipboardRecorder } from "./web-feedback";

function remoteNotice(page: Page) {
	return page.getByRole("region", { name: "Remote server notice", exact: true });
}

/** Server signals and the workspace notice for a selected remote (ADR 0031). */
export const remoteActions = {
	async remoteSessionSignal(page, step) {
		await page.evaluate(`import('/src/lib/settings-integration-mocks.ts').then(m => m.remoteSignalMock(${JSON.stringify(step.state)}))`);
	},
	async expectRemoteNotice(page, step) {
		await remoteNotice(page).getByText(step.value, { exact: false }).first().waitFor();
	},
	async expectNoRemoteNotice(page) {
		await remoteNotice(page).waitFor({ state: "detached" });
	},
	async clickRemoteNoticeButton(page, step) {
		// The page's clipboard records the copied link instead of needing a permission.
		if (step.label === "Copy sign-in link") await page.evaluate(clipboardRecorder);
		await remoteNotice(page).getByRole("button", { name: step.label, exact: true }).click();
	},
} satisfies Partial<WebActions>;
