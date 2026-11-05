import type { Page } from "playwright-core";
import type { Step } from "./contract";

/** Secret reveal, YAML error lens, diagnostics and documentation actions. */
export async function adapterAction(page: Page, step: Step): Promise<void> {
    switch (step.action) {
        case "cancelYamlEdit": await page.getByRole("button", { name: "Cancel", exact: true }).click(); break;
        case "revealSecretValue": case "hideSecretValue":
            await page.getByRole("button", { name: step.action === "revealSecretValue" ? "Reveal" : "Hide", exact: true }).first().click(); break;
        case "expectSecretValue": {
            const value = page.locator("dd").filter({ hasText: /^secret$/ });
            await value.waitFor({ state: step.visible ? "visible" : "hidden" });
            break;
        }
        case "expectYamlLens": {
            const lens = page.locator(".cm-yaml-error-lens");
            if (step.line === 0) await lens.waitFor({ state: "hidden" });
            else {
                await lens.first().waitFor();
                const line = await lens.first().evaluate((node) => {
                    const previous = node.closest(".cm-widgetBuffer") ?? node.parentElement;
                    return previous?.textContent;
                });
                if (!line) throw new Error("Missing inline YAML diagnostic");
                const marker = page.locator(".cm-lintRange-error").first();
                await marker.waitFor();
                const text = await marker.evaluate((node) => node.closest(".cm-line")?.textContent);
                const marked = step.marked ?? "metadata:";
                if (!text?.includes(marked)) throw new Error("YAML diagnostic marked the wrong line");
            }
            break;
        }
        case "refreshLatencyReport": case "clearLatencyReport":
            await page.getByText("Latency report", { exact: true }).locator("xpath=../..")
                .getByRole("button", { name: step.action === "refreshLatencyReport" ? "Refresh" : "Clear", exact: true }).click(); break;
        case "expectLatencyEmpty":
            await page.getByText("No timed frontend spans yet.", { exact: true }).waitFor(); break;
        case "expectLatencyTiming": {
            if (step.command !== "get_resource_yaml") throw new Error("Unknown timed command");
            const row = page.getByRole("row").filter({ has: page.getByRole("cell", { name: "detail.yaml.fetch.done", exact: true }) });
            await row.waitFor();
            if (!(await row.textContent())?.includes("ms")) throw new Error("Missing measured duration");
            break;
        }
        case "openDocumentation":
            await page.evaluate(() => {
                document.addEventListener("click", (event) => {
                    const link = event.target instanceof Element ? event.target.closest("a") : null;
                    if (link?.target === "_blank") {
                        event.preventDefault();
                        document.documentElement.dataset.documentationRequest = link.href;
                    }
                }, { capture: true });
            });
            await page.getByRole("link", { name: "Open Wiki", exact: true }).click(); break;
        case "expectDocumentationRequest":
            if (await page.locator("html").getAttribute("data-documentation-request") !== step.url) throw new Error("Wrong documentation URL");
            break;
        default: throw new Error(`Not an adapter action: ${step.action}`);
    }
}
