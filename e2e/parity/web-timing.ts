import type { Page } from "playwright-core";
import { waitFor } from "../../.agents/skills/verify-kubecove/scripts/processes";
import type { Step } from "./contract";

declare global {
    interface Window { __kubecoveTiming: import("../../src/lib/tauri-dev-mock-timing").MockTiming; }
    interface Window { paritySettingsMocks: typeof import("../../src/lib/settings-integration-mocks"); }
}

export async function timingAction(page: Page, step: Step) {
    switch (step.action) {
        case "expectTopologyDenied": await page.getByRole("alert").getByText("KubeCove does not have permission for this", { exact: true }).waitFor(); break;
        case "expectWatchStart": {
            const started = () => page.evaluate(`import('/src/lib/tauri-dev-mock-live.ts').then(m => m.lastWatchStart(${JSON.stringify(step.kind)}))`) as Promise<{ resourceVersion?: string } | null>;
            await waitFor(`${step.kind} watch start`, async () => (await started()) !== null);
            const version = (await started())?.resourceVersion ?? null;
            if (version !== step.resourceVersion) throw new Error(`${step.kind} watch started at ${version}, expected ${step.resourceVersion}`);
            break;
        }
        case "mockTiming":
            await page.evaluate("import('/src/lib/tauri-dev-mock-timing.ts').then(m => { globalThis.__kubecoveTiming = m.mockTiming; })");
            if (step.control.operation === "fail" && step.control.kind === "forbidden") await page.clock.install();
            if (step.control.operation === "expectNoRetry") {
                // @tanstack/query-core/src/retryer.ts caps defaultRetryDelay at 30,000 ms.
                // Diagnostics polls every 1500 ms. Observe the first retry at 1000 ms before that independent poll.
                await page.clock.runFor(step.control.command === "get_backend_diagnostics" ? 1000 : 30_000);
                await page.clock.resume();
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
            break;
        default: throw new Error(`Unknown timing action ${step.action}`);
    }
}
