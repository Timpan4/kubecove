import type { Page } from "playwright-core";
import type { Step } from "./contract";

/** The developer topology harness that Diagnostics opens in development builds. */
export async function topologySpikeAction(page: Page, step: Step): Promise<void> {
    switch (step.action) {
        case "runTopologySpike":
            await page.getByRole("button", { name: "Run benchmark", exact: true }).click();
            // The run is finite, 100 selections, 24 viewport moves and a fit, but at 4,000 nodes
            // headless Chromium spends about 50 s on it, past the default wait. No project limit
            // covers a benchmark run, so the wait is unbounded.
            await page.waitForFunction(() => (document.querySelector("#result")?.textContent ?? "").length > 0, null, { timeout: 0 });
            break;
        case "expectTopologySpikeText":
            // The header splits its counts across source lines, so whitespace is compared collapsed.
            await page.waitForFunction(
                (value) => (document.body.textContent ?? "").replace(/\s+/g, " ").includes(value),
                step.value,
            );
            break;
        default: throw new Error(`Not a topology spike action: ${step.action}`);
    }
}
