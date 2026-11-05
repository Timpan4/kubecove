import type { Page } from "playwright-core";
import type { Step } from "./contract";

function forwardRows(page: Page, target: string) {
    const table = page.locator("table").filter({ has: page.getByRole("columnheader", { name: "Target", exact: true }) });
    return table.locator("tbody tr").or(page.locator("div.rounded-md.border.bg-background.p-3")).filter({ hasText: target });
}
export async function liveSessionAction(page: Page, step: Step): Promise<void> {
    switch (step.action) {
        case "configurePreset":
            for (const [label, value] of [["label", step.label], ["cluster context", step.context], ["namespace", step.namespace], ["Service name", step.service], ["Service port", step.remote], ["local port", step.local]]) await page.getByRole("textbox", { name: `Saved forward ${label}`, exact: true }).fill(value ?? "");
            break;
        case "presetControl": {
            const rows = page.locator("table").filter({ has: page.getByRole("columnheader", { name: "Preset", exact: true }) }).locator("tbody tr").filter({ hasText: step.label ?? "" });
            if (step.operation === "add") await page.getByRole("button", { name: "Save forward", exact: true }).click();
            else if (step.operation === "save") {
                const edit = page.getByRole("button", { name: "Save changes", exact: true });
                if (await edit.isVisible()) await edit.click(); else await page.getByRole("button", { name: "Save forward", exact: true }).last().click();
            } else if (step.operation === "absent") await rows.waitFor({ state: "hidden" });
            else await rows.getByRole("button", { name: step.operation === "remove" ? "Delete" : step.operation === "edit" ? "Edit" : "Start", exact: true }).click();
            break;
        }
        case "replaySessions": await page.evaluate(`import('/src/lib/tauri-dev-mock-session-controls.ts').then(m => m.replaySessions(${JSON.stringify(step.script)}))`); break;
        case "confirmExecLeave": await page.getByRole("button", { name: step.choice === "stay" ? "Stay" : "Stop and leave", exact: true }).click(); break;
        case "configureForward": {
            const remote = page.locator('input[placeholder="8080"]');
            if (await remote.count()) await remote.fill(step.remote);
            else {
                await page.getByRole("combobox").last().click();
                await page.getByRole("option").filter({ hasText: step.remote }).first().click();
            }
            await page.locator('input[placeholder="auto"]').fill(step.local);
            break;
        }
        case "forwardControl":
            if (step.operation === "start") await page.getByRole("button", { name: "Start", exact: true }).click();
            else if (step.operation === "save") await page.getByRole("button", { name: "Save preset", exact: true }).click();
            else if (step.operation === "restore") await page.getByRole("button", { name: "Start saved", exact: true }).first().click();
            else await forwardRows(page, step.target ?? "").getByRole("button", { name: step.operation === "stop" ? "Stop" : "Reconnect", exact: true }).click();
            break;
        case "expectForwardState": {
            const rows = forwardRows(page, step.target);
            if (step.count === 0) await rows.waitFor({ state: "hidden" });
            else {
                await rows.getByText(step.status, { exact: true }).waitFor();
                if (await rows.count() !== step.count) throw new Error("Forward duplicated or missing");
                await rows.getByText(`127.0.0.1:${step.local}`, { exact: false }).first().waitFor();
                if (step.resolved) await rows.getByText(step.resolved, { exact: false }).first().waitFor();
            }
            break;
        }
        case "execControl":
            if (step.operation === "stop") await page.getByRole("button", { name: "Stop session", exact: true }).click();
            else if (step.operation === "custom") {
                await page.getByRole("button", { name: "Other", exact: true }).click();
                await page.getByRole("button", { name: "Custom command…", exact: true }).click();
                await page.getByRole("combobox", { name: "Exec container", exact: true }).click();
                await page.getByRole("option", { name: step.container, exact: true }).click();
            }
            else if (step.operation === "arguments") await page.getByPlaceholder("/usr/bin/env\nprintenv", { exact: true }).fill(step.command ?? "");
            else if (step.operation === "run") await page.getByRole("button", { name: `Run ${step.command} in ${step.container}`, exact: true }).click();
            else if (step.operation === "start") {
                const start = page.getByRole("button", { name: `Open ${step.command} in ${step.container}`, exact: true }).last();
                if (!(await start.isVisible())) {
                    const menu = page.getByRole("button", { name: "New exec session", exact: true });
                    if (await menu.isVisible()) await menu.click();
                    else await page.getByRole("button", { name: "Other", exact: true }).click();
                }
                await start.click();
            }
            else if (step.operation === "select") await page.getByRole("button", { name: new RegExp(` · ${step.container}, running$`) }).click();
            else if (step.operation === "close") {
                const close = page.getByRole("button", { name: /^Stop and close / });
                if (step.container) await close.and(page.getByRole("button", { name: new RegExp(` · ${step.container}$`) })).click();
                else await close.click();
            }
            else {
                await page.locator(".xterm-helper-textarea").focus();
                const parts = (step.data ?? "").split("\r");
                for (const [index, part] of parts.entries()) {
                    if (part) await page.keyboard.type(part);
                    if (index < parts.length - 1) await page.keyboard.press("Enter");
                }
            }
            break;
        case "expectExecState":
            if (step.status === "closed") await page.getByText("No exec sessions", { exact: true }).waitFor();
            else {
                await page.getByRole("button", { name: new RegExp(`${step.command ?? ""} · ${step.container}, ${step.status}$`) }).waitFor();
                await page.locator(".xterm:visible .xterm-rows").getByText(step.contains, { exact: false }).first().waitFor();
                if (step.absent && (await page.locator(".xterm:visible .xterm-rows").textContent())?.includes(step.absent)) throw new Error("Output leaked from another session");
            }
            break;
    }
}
