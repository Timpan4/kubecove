import type { Page } from "playwright-core";
import { waitFor } from "../../.agents/skills/verify-kubecove/scripts/processes";
import type { Step } from "./contract";

declare global {
    interface Window { __kubecoveTiming: import("../../src/lib/tauri-dev-mock-timing").MockTiming; }
    interface Window { paritySettingsMocks: typeof import("../../src/lib/settings-integration-mocks"); }
}

export async function timingAction(page: Page, step: Step, scenarioSteps: readonly Step[]) {
    switch (step.action) {
        case "expectTopologyDenied": await page.getByRole("alert").getByText("KubeCove does not have permission for this", { exact: true }).waitFor(); break;
        case "expectWatchStart": {
            const started = () => page.evaluate<{ resourceVersion?: string } | null>(`import('/src/lib/tauri-dev-mock-live.ts').then(m => m.lastWatchStart(${JSON.stringify(step.kind)}))`);
            await waitFor(`${step.kind} watch start`, async () => (await started()) !== null);
            const version = (await started())?.resourceVersion ?? null;
            if (version !== step.resourceVersion) throw new Error(`${step.kind} watch started at ${version}, expected ${step.resourceVersion}`);
            break;
        }
        case "mockTiming":
            await page.evaluate("import('/src/lib/tauri-dev-mock-timing.ts').then(m => { globalThis.__kubecoveTiming = m.mockTiming; })");
            if ((step.control.operation === "fail" || step.control.operation === "failOnce") && scenarioSteps.some((candidate) => candidate.action === "mockTiming" && candidate.control.operation === "expectNoRetry" && candidate.control.command === step.control.command && candidate.control.context === step.control.context)) {
                try {
                    await page.clock.pauseAt(await page.evaluate(() => Date.now()));
                } catch (error) {
                    // Playwright pauses before rejecting a stale timestamp, so the second read cannot drift.
                    if (!String(error).includes("Cannot fast-forward to the past")) throw error;
                    await page.clock.pauseAt(await page.evaluate(() => Date.now()));
                }
            }
            if (step.control.operation === "expectNoRetry") {
                // TanStack's defaultRetryDelay starts at 1000 ms. Later polls are separate reads.
                await page.clock.runFor(1000);
            }
            if (step.control.operation === "expectNoError") {
                if (await page.getByText("Scripted read:", { exact: false }).count()) throw new Error("Read error shown while retry is pending");
            }
            if (step.control.operation === "expectPending") {
                await waitFor("pending mock read", async () => page.evaluate((control) => window.__kubecoveTiming.pending(control.command, control.context, control.namespace), step.control));
            }
            if (step.control.operation === "expectRequests") {
                await waitFor("mock request count", async () => page.evaluate((control) => window.__kubecoveTiming.requestCount(control.command, control.context) === control.count, step.control));
            }
            await page.evaluate(`import('/src/lib/tauri-dev-mock-timing.ts').then(m => m.mockTiming.control(${JSON.stringify(step.control)}))`);
            if (step.control.operation === "expectNoRetry") await page.clock.resume();
            break;
        default: throw new Error(`Unknown timing action ${step.action}`);
    }
}
