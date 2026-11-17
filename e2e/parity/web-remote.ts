import type { WebActions } from "./web-action";
import { clipboardRecorder } from "./web-feedback";

/** Server signals and the workspace notice for a selected remote (ADR 0031). */
export const remoteActions = {
	async remoteSessionSignal(page, step) {
		await page.evaluate(`import('/src/lib/settings-integration-mocks.ts').then(m => m.remoteSignalMock(${JSON.stringify(step.state)}))`);
	},
	async expectRemoteNotice(page, step) {
		const notice = page.getByRole("region", { name: "Remote server notice", exact: true });
		await notice.getByText(step.value, { exact: false }).first().waitFor();
	},
	async expectNoRemoteNotice(page) {
		const notice = page.getByRole("region", { name: "Remote server notice", exact: true });
		await notice.waitFor({ state: "detached" });
	},
	async clickRemoteNoticeButton(page, step) {
		const notice = page.getByRole("region", { name: "Remote server notice", exact: true });
		// The page's clipboard records the copied link instead of needing a permission.
		if (step.label === "Copy sign-in link") await page.evaluate(clipboardRecorder);
		await notice.getByRole("button", { name: step.label, exact: true }).click();
	},
} satisfies Partial<WebActions>;
