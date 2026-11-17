import type { Page } from "playwright-core";
import { MCP_FIXTURE_TOKEN, MCP_ROTATED_TOKEN } from "../../src/lib/mcp-dev-mocks";
import { claudeCodeCommand } from "../../src/lib/mcp-runtime";
import type { Step } from "./contract";

export async function mcpAction(page: Page, step: Step): Promise<void> {
    switch (step.action) {
        case "fillMcpPort": await page.getByRole("textbox", { name: "MCP port", exact: true }).fill(step.value); break;
        case "mcpControl":
            if (step.control.startsWith("Copy")) {
                await page.evaluate(() => {
                    Object.defineProperty(navigator, "clipboard", { configurable: true, value: {
                        writeText: async (value: string) => { document.documentElement.dataset.mcpCopy = value; },
                    } });
                });
            }
            if (step.control === "Reload settings") {
                // A reload that fails again leaves the switch disabled and shows the same button,
                // so wait for the mock to log the reload before reading the settled controls.
                const sent = (await mcpRequests(page)).length;
                await page.getByRole("button", { name: step.control, exact: true }).click();
                await page.waitForFunction(async (count) => {
                    // @ts-expect-error: Vite serves this source module to the browser.
                    const mocks = await import("/src/lib/mcp-dev-mocks.ts");
                    return mocks.mcpMockRequests().length > count;
                }, sent);
                await page.waitForFunction(() => !document.querySelector('button[role="switch"][aria-label="MCP server"]')?.hasAttribute("disabled")
                    || [...document.querySelectorAll("button")].some((button) => button.textContent?.trim() === "Reload settings" && !button.disabled));
                break;
            }
            if (step.control === "Submit port") await page.getByRole("textbox", { name: "MCP port", exact: true }).press("Enter");
            else await page.getByRole(step.control === "MCP server" ? "switch" : "button", { name: step.control, exact: true }).click();
            // The status switch is disabled throughout each request, including token copies.
            await page.waitForFunction(() => !document.querySelector('button[role="switch"][aria-label="MCP server"]')?.hasAttribute("disabled"));
            break;
        case "configureMcpSettingsFile":
            await page.evaluate(`import('/src/lib/mcp-dev-mocks.ts').then(m => m.configureMcpSettingsFile(${JSON.stringify(step.file)}))`);
            break;
        case "expectMcpLoadError": {
            const reload = page.getByRole("button", { name: "Reload settings", exact: true });
            await reload.waitFor();
            if (!(await reload.isEnabled())) throw new Error("Reload settings is disabled");
            await page.getByRole("alert").filter({ hasText: step.text }).waitFor();
            if (await page.getByRole("switch", { name: "MCP server", exact: true }).isEnabled()) throw new Error("MCP controls enabled without settings");
            break;
        }
        case "expectMcpState": {
            const toggle = page.getByRole("switch", { name: "MCP server", exact: true });
            await page.waitForFunction(() => !document.querySelector('button[role="switch"][aria-label="MCP server"]')?.hasAttribute("disabled"));
            if ((await toggle.getAttribute("aria-checked")) !== String(step.enabled)) throw new Error("Wrong MCP enabled state");
            if (await page.getByRole("textbox", { name: "MCP port", exact: true }).inputValue() !== step.port) throw new Error("Wrong MCP port");
            if (await page.getByRole("button", { name: "Save", exact: true }).isEnabled() !== step.save) throw new Error("Wrong MCP Save state");
            if (await page.getByRole("button", { name: "Copy Claude Code command", exact: true }).isEnabled() !== step.command) throw new Error("Wrong MCP command state");
            if (step.running) await page.getByText(`Listening at http://127.0.0.1:${step.port}/mcp with 11 read-only tools.`, { exact: true }).waitFor();
            else if (await page.getByText("Running", { exact: true }).count()) throw new Error("MCP unexpectedly running");
            if (await page.getByRole("button", { name: "Reload settings", exact: true }).count()) throw new Error("Reload settings shown with loaded settings");
            break;
        }
        case "expectMcpCopy": {
            const copied = await page.locator("html").getAttribute("data-mcp-copy");
            const token = step.rotated ? MCP_ROTATED_TOKEN : MCP_FIXTURE_TOKEN;
            const expected = step.kind === "token" ? token : claudeCodeCommand("http://127.0.0.1:47615/mcp", token);
            if (copied !== expected) throw new Error("Wrong MCP clipboard content");
            await page.getByText(step.kind === "token" ? "Copied token." : "Copied Claude Code command.", { exact: true }).waitFor();
            if ((await page.locator("body").textContent())?.includes(token)) throw new Error("MCP credential rendered");
            if (await page.evaluate((value) => Object.values(localStorage).some((entry) => String(entry).includes(value)), token)) throw new Error("MCP credential persisted in preferences");
            break;
        }
        case "expectMcpRequests": {
            const requests = await mcpRequests(page);
            if (JSON.stringify(requests) !== JSON.stringify(step.requests)) throw new Error(`Wrong MCP request sequence: ${JSON.stringify(requests)}`);
            break;
        }
        default: throw new Error(`Not an MCP action: ${step.action}`);
    }
}

function mcpRequests(page: Page): Promise<string[]> {
    return page.evaluate(async () => {
        // @ts-expect-error: Vite serves this source module to the browser.
        const mocks = await import("/src/lib/mcp-dev-mocks.ts");
        return mocks.mcpMockRequests();
    });
}
