import type { Page } from "playwright-core";
import type { Step } from "./contract";
export async function argoAction(page: Page, step: Step) {
    switch (step.action) {
        case "reconnectArgoProfile":
            await page.getByRole("tabpanel").getByRole("button", { name: /^(Manage connection|Connect Argo CD)$/ }).click();
            await page.getByRole("dialog").getByRole("button", { name: "Reconnect", exact: true }).click();
            break;
        case "expandArgoDiff": await page.getByRole("button", { name: /^Show all .* lines$/ }).first().click(); break;
        case "configureArgoInventory":
            await page.evaluate(`import('/src/lib/tauri-dev-mock-argo.ts').then(m => m.configureArgoInventory(${JSON.stringify(step.mode)}))`);
            await page.getByRole("button", { name: "Live updates and refresh options", exact: true }).first().click();
            await page.getByRole("button", { name: "Refresh (clear cache)", exact: true }).click();
            await page.keyboard.press("Escape");
            break;
        case "copyArgoValue": {
            await page.evaluate(() => Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText: async (value: string) => { document.documentElement.dataset.argoCopy = value; } } }));
            if (step.control.endsWith("value")) await page.getByRole("button", { name: new RegExp(`^Copy ${step.control}:`) }).click();
            else {
                const suffix = step.control === "source" ? "Activate to copy source details." : "Activate to copy.";
                const card = page.locator("div.group.relative").filter({ has: page.getByRole("button", { name: "Open details for Application platform-cilium in argocd", exact: true }) });
                await card.locator(`[aria-label$="${suffix}"]`).filter({ hasText: step.control === "source" ? "" : step.contains }).first().click();
            }
            if (!(await page.locator("html").getAttribute("data-argo-copy"))?.includes(step.contains)) throw new Error("Wrong Argo clipboard value");
            break;
        }
        case "configureArgoMock":
            await page.evaluate(`import('/src/lib/tauri-dev-mock-argo.ts').then(m => m.configureArgoMock(${JSON.stringify(step.script)}))`);
            break;
        case "focusArgoResource":
            await page.getByRole("tabpanel").getByRole("button", { name: step.label, exact: false }).first().click();
            break;
        case "expectNoSavedArgoProfile": {
            await page.getByRole("tabpanel").getByRole("button", { name: /^(Manage connection|Connect Argo CD)$/ }).click();
            await page.getByRole("dialog", { name: "Argo CD connection" }).getByText(step.url, { exact: true }).waitFor({ state: "hidden" });
            await page.keyboard.press("Escape");
            break;
        }
        case "argoControl": {
            if (step.control === "Technical detail") {
                await page.getByText("Technical detail", { exact: true }).first().click();
            } else if (step.control === "Open connection") {
                await page.getByRole("tabpanel").getByRole("button", { name: /^(Manage connection|Connect Argo CD)$/ }).click();
                await page.getByRole("dialog").getByRole("radio", { name: "Found in cluster", exact: true }).click();
            } else if (step.control === "Close connection") {
                await page.keyboard.press("Escape");
            } else if (step.control === "Forget") {
                await page.getByRole("tabpanel").getByRole("button", { name: /^(Manage connection|Connect Argo CD)$/ }).click();
                const dialog = page.getByRole("dialog", { name: "Argo CD connection" });
                await dialog.getByRole("button", { name: "Forget", exact: true }).click();
                await dialog.getByRole("button", { name: "Forget", exact: true }).waitFor({ state: "hidden" });
                await page.keyboard.press("Escape");
            } else await page.getByRole("button", { name: step.control, exact: true }).first().click();
            break;
        }
        case "connectArgoProfile": {
            await (step.surface === "page" ? page.getByRole("button", { name: "Argo CD connection", exact: true }) : page.getByRole("tabpanel").getByRole("button", { name: "Connect Argo CD", exact: true })).click();
            const dialog = page.getByRole("dialog", { name: "Argo CD connection" });
            if (step.tunnel) {
                await dialog.getByRole("radio", { name: "Found in cluster", exact: true }).click();
                await dialog.getByRole("radio", { name: /argocd\/argocd-server:443/ }).click();
            } else {
                await dialog.getByRole("radio", { name: "External URL", exact: true }).click();
                await dialog.getByLabel("Argo CD server URL", { exact: true }).fill(step.url);
            }
            if (step.local) {
                await dialog.getByRole("radio", { name: "Local login", exact: true }).click();
                await dialog.getByLabel("Username", { exact: true }).fill("fixture-user");
                await dialog.getByLabel("Password", { exact: true }).fill("fixture-password");
            } else await dialog.getByLabel("Token", { exact: true }).fill("parity-fixture-token");
            if (step.insecure || step.customCa) {
                await dialog.getByText("Advanced", { exact: true }).click();
                if (step.customCa) await dialog.getByLabel("Custom CA PEM (session only)", { exact: true }).fill(step.customCa);
                if (step.insecure) {
                    await dialog.getByRole("switch").first().click();
                    await dialog.getByText("Certificate validation is disabled and never saved.", { exact: true }).waitFor();
                }
            }
            if (step.remember) await dialog.getByRole("switch", { name: "Remember credential in native keyring", exact: true }).click();
            await dialog.getByRole("button", { name: "Connect", exact: true }).click();
            if (step.tunnel) { await dialog.getByText("Confirm tunnel target", { exact: true }).waitFor(); break; }
            await dialog.getByRole("button", { name: "Disconnect", exact: true }).waitFor();
            await page.keyboard.press("Escape");
            break;
        }
        case "disconnectArgoProfile": {
            await page.getByRole("tabpanel").getByRole("button", { name: "Manage connection", exact: true }).click();
            const dialog = page.getByRole("dialog", { name: "Argo CD connection" });
            await dialog.getByRole("button", { name: "Disconnect", exact: true }).click();
            await dialog.getByRole("button", { name: "Reconnect", exact: true }).waitFor();
            await page.keyboard.press("Escape");
            break;
        }
        case "reviewArgoOperation":
            if (step.operation === "Hard refresh") {
                await page.getByRole("button", { name: "Refresh options", exact: true }).click();
                await page.getByRole("button", { name: "Hard refresh", exact: true }).click();
            } else await page.getByRole("tabpanel").getByRole("button", { name: step.operation, exact: true }).click();
            break;
        case "confirmArgoReview": await page.getByRole("button", { name: "Confirm Argo CD operation", exact: true }).click(); break;
        case "cancelArgoReview": await page.getByRole("button", { name: "Cancel review", exact: true }).click(); break;
        case "expectArgoReview": {
            const review = page.getByRole("region", { name: "Review Argo CD operation" });
            await review.getByText(`${step.operation} ${step.transport} mock-dev Application ${step.namespace} ${step.name}`, { exact: true }).waitFor();
            break;
        }
        case "expectArgoText": {
            const text = page.getByText(step.text, { exact: false }).first();
            await text.waitFor({ state: "attached" });
            const technical = page.locator("details").filter({ has: text });
            if (await technical.count() && !(await text.isVisible())) await technical.locator("summary").click();
            await text.waitFor();
            break;
        }
        case "argoDocument":
            await page.getByRole("tabpanel").getByRole("radio", { name: step.view, exact: true }).click();
            await page.getByRole("tabpanel").getByText(step.contains, { exact: false }).first().waitFor();
            break;
        case "confirmArgoTunnel": {
            const dialog = page.getByRole("dialog", { name: "Argo CD connection" });
            await dialog.getByText("argocd-server-7d9f8b6c5-x2k4q", { exact: true }).waitFor();
            await dialog.getByText("SHA-256", { exact: false }).waitFor();
            await dialog.getByRole("button", { name: "Confirm and connect", exact: true }).click();
            await dialog.getByRole("button", { name: "Disconnect", exact: true }).waitFor();
            await page.keyboard.press("Escape"); break;
        }
        case "selectArgoPreference":
            await page.getByRole("combobox", { name: "Argo CD inspection preference", exact: true }).click();
            await page.getByRole("option", { name: step.preference === "Kubernetes" ? "Kubernetes" : "Automatic", exact: false }).click();
            break;
        case "expectSavedArgoProfile":
            await page.getByRole("tabpanel").getByRole("button", { name: /^(Manage connection|Connect Argo CD)$/ }).click();
            await page.getByRole("dialog", { name: "Argo CD connection" }).getByText(step.url, { exact: true }).waitFor();
            await page.keyboard.press("Escape");
            break;
        case "configureArgoSync": {
            await page.getByRole("button", { name: "Sync options", exact: true }).click();
            const options = page.locator("[data-slot=popover-content]");
            await options.getByRole("textbox").fill(step.revision);
            await options.getByRole("checkbox", { name: "Prune", exact: true }).click();
            await options.getByRole("checkbox", { name: "Dry run", exact: true }).click();
            await options.getByRole("checkbox", { name: "Force / replace", exact: true }).click();
            await options.getByRole("button", { name: "Sync with these options", exact: true }).click();
            const confirmation = page.getByRole("dialog", { name: "Confirm advanced sync" });
            await confirmation.getByRole("textbox").fill(step.name);
            await confirmation.getByRole("button", { name: "Confirm sync", exact: true }).click();
            break;
        }
        case "selectArgoHistory":
            await page.getByRole("combobox", { name: "Recent deployment", exact: true }).click();
            await page.getByRole("option", { name: step.revision, exact: true }).click();
            break;
        case "expectArgoWorkspaceTransport": await page.getByText(step.text, { exact: false }).first().waitFor(); break;
        case "completeArgoOperation":
            await page.evaluate("import('/src/lib/tauri-dev-mock-argo.ts').then(m => m.completeMockArgoOperation())");
            break;

        default: throw new Error(`Unknown Argo action ${step.action}`);
    }
}
