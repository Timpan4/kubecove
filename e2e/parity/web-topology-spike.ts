import type { Page } from "playwright-core";
import type { Step } from "./contract";

/** The developer topology harness that Diagnostics opens in development builds. */
export async function topologySpikeAction(page: Page, step: Step): Promise<void> {
    switch (step.action) {
        case "runTopologySpike": {
            // A run that throws never publishes #result; its unhandled rejection reaches the page
            // as an error, which ends the wait instead.
            let onError = (_error: Error) => {};
            const failed = new Promise<never>((_, reject) => {
                onError = (error) => reject(new Error(`Topology benchmark failed: ${error.message}`));
                page.on("pageerror", onError);
            });
            // Whichever promise loses the race settles later with nobody awaiting it.
            failed.catch(() => {});
            try {
                await page.getByRole("button", { name: "Run benchmark", exact: true }).click();
                // The run is finite, 100 selections, 24 viewport moves and a fit, but at 4,000 nodes
                // headless Chromium spends about 50 s on it, past the default wait. No project limit
                // covers a benchmark run, so the wait has no timeout.
                const published = page.waitForFunction(() => (document.querySelector("#result")?.textContent ?? "").length > 0, null, { timeout: 0 });
                published.catch(() => {});
                await Promise.race([published, failed]);
            } finally { page.off("pageerror", onError); }
            break;
        }
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
