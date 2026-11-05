import type { Page } from "playwright-core";
import type { Step } from "./contract";

export async function argoAction(page: Page, step: Step): Promise<void> {
    switch (step.action) {
        case "connectArgoProfile": {
            await (step.surface === "page" ? page.getByRole("button", { name: "Argo CD connection", exact: true }) : page.getByRole("tabpanel").getByRole("button", { name: "Connect Argo CD", exact: true })).click();
            const dialog = page.getByRole("dialog", { name: "Argo CD connection" });
            await dialog.getByRole("radio", { name: "External URL", exact: true }).click();
            await dialog.getByLabel("Argo CD server URL", { exact: true }).fill(step.url);
            await dialog.getByLabel("Token", { exact: true }).fill("parity-fixture-token");
            await dialog.getByRole("button", { name: "Connect", exact: true }).click();
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
        case "reviewArgoOperation": await page.getByRole("tabpanel").getByRole("button", { name: step.operation, exact: true }).click(); break;
        case "confirmArgoReview": await page.getByRole("button", { name: "Confirm Argo CD operation", exact: true }).click(); break;
        case "cancelArgoReview": await page.getByRole("button", { name: "Cancel review", exact: true }).click(); break;
        case "expectArgoReview": {
            const review = page.getByRole("region", { name: "Review Argo CD operation" });
            await review.getByText(`${step.operation} ${step.transport} mock-dev Application ${step.namespace} ${step.name}`, { exact: true }).waitFor();
            break;
        }
        case "expectArgoText": await page.getByRole("tabpanel").getByText(step.text, { exact: false }).first().waitFor(); break;
        case "argoDocument":
            await page.getByRole("tabpanel").getByRole("radio", { name: step.view, exact: true }).click();
            await page.getByRole("tabpanel").getByText(step.contains, { exact: false }).first().waitFor();
            break;
        default: throw new Error(`Not an Argo CD action: ${step.action}`);
    }
}
