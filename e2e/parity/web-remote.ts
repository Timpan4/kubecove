import type { Page } from "playwright-core";
import type { Step } from "./contract";
import { clipboardRecorder } from "./web-feedback";

/** Server signals and the workspace notice for a selected remote (ADR 0031). */
export async function remoteAction(page: Page, step: Step): Promise<void> {
    const notice = page.getByRole("region", { name: "Remote server notice", exact: true });
    switch (step.action) {
        case "remoteSessionSignal":
            await page.evaluate(`import('/src/lib/settings-integration-mocks.ts').then(m => m.remoteSignalMock(${JSON.stringify(step.state)}))`);
            break;
        case "expectRemoteNotice": await notice.getByText(step.value, { exact: false }).first().waitFor(); break;
        case "expectNoRemoteNotice": await notice.waitFor({ state: "detached" }); break;
        case "clickRemoteNoticeButton":
            // The page's clipboard records the copied link instead of needing a permission.
            if (step.label === "Copy sign-in link") await page.evaluate(clipboardRecorder);
            await notice.getByRole("button", { name: step.label, exact: true }).click(); break;
        default: throw new Error(`Unhandled remote action ${step.action}`);
    }
}
