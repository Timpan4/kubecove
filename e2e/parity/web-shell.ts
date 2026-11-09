import type { Page } from "playwright-core";
import { STARTUP_HOLDS_KEY, type TimingControl } from "../../src/lib/tauri-dev-mock-timing";
import type { Scenario, Step } from "./contract";

/** The web driver creates its workspace through the launcher unless a scenario starts without one. */
export function seedsWorkspace(scenario: Scenario): boolean {
    return scenario.steps[0]?.action !== "startWithoutWorkspace";
}

/**
 * A shell phase other than ready reloads the page, and the reloaded app reads at once. The holds
 * that follow the phase step are stored for the timing mock to apply before those reads, as the
 * native driver applies leading phase and hold steps before its client starts.
 */
export async function storeReloadHolds(page: Page, steps: Step[], index: number): Promise<void> {
    const step = steps[index];
    if (step?.action !== "configureShellRead" || step.phase === "ready") return;
    const holds: TimingControl[] = [];
    for (const next of steps.slice(index + 1)) {
        if (next.action !== "mockTiming" || next.control.operation !== "hold") break;
        holds.push(next.control);
    }
    if (holds.length) await page.evaluate(([key, value]) => sessionStorage.setItem(key, value), [STARTUP_HOLDS_KEY, JSON.stringify(holds)]);
}

export async function seedWorkspace(page: Page): Promise<void> {
    await page.locator("#workspace-name").fill("Parity mock workspace");
    await page.getByRole("button", { name: "Create workspace", exact: true }).click();
    // Creating a workspace navigates to its overview; a dialog opened before that navigation lands is closed by it.
    await page.waitForURL(/#\/workspace\/[^/]+\/overview$/);
}

export async function shellAction(page: Page, step: Step): Promise<void> {
    switch (step.action) {
        case "startWithoutWorkspace":
            await page.getByRole("button", { name: "Create workspace", exact: true }).waitFor();
            break;
        case "expectNoWorkspaceShell":
            await page.getByRole("button", { name: "Search views, namespaces, and resources", exact: true }).waitFor({ state: "detached" });
            if (await page.getByRole("tree").count()) throw new Error("Sidebar rendered without an open workspace");
            break;
        case "expectPaletteUnavailable":
            await page.keyboard.press("ControlOrMeta+K");
            if (await page.getByRole("dialog").count()) throw new Error("Palette opened without an open workspace");
            break;
        case "openLauncher": {
            // Wait for either view: a launcher that renders after a one-off visibility check is still the launcher.
            const launcher = page.getByRole("button", { name: "Create workspace", exact: true });
            const open = page.getByRole("button", { name: "Open workspaces", exact: true });
            await launcher.or(open).first().waitFor();
            if (!await launcher.isVisible()) await open.click();
            break;
        }
        case "expectOverviewShortcuts": case "overviewShortcut": {
            const section = page.getByRole("heading", { name: "Shortcuts", exact: true }).locator("xpath=ancestor::section[1]");
            if (step.action === "overviewShortcut") await section.getByRole("button", { name: step.label, exact: true }).click();
            else {
                const labels = await section.getByRole("button").allTextContents();
                if (JSON.stringify(labels.map(label => label.trim())) !== JSON.stringify(step.labels)) throw new Error("Saved shortcuts differ");
            }
            break;
        }
        case "expectOverviewScope":
            for (const label of step.labels) await page.locator("section > header").getByText(label, { exact: true }).waitFor();
            break;
        case "expectCompareTotal": {
            const comparison = page.getByText(step.title, { exact: true }).locator("..");
            const side = comparison.getByText(step.side, { exact: true }).locator("..");
            await side.getByText(new RegExp(`^${step.total} total /`)).waitFor();
            break;
        }
        case "retryOverview": await page.getByRole("button", { name: "Retry workspace resources", exact: true }).click(); break;
        case "expectOverviewRetry": await page.getByRole("button", { name: "Retry workspace resources", exact: true }).waitFor(); break;
        case "dragKind": {
            const source = page.locator(`[data-kind-drop="${step.kind}"]`);
            const target = page.locator(`[data-kind-drop="${step.target}"]`);
            await source.scrollIntoViewIfNeeded();
            const from = await source.boundingBox();
            const to = await target.boundingBox();
            if (!from || !to) throw new Error("Missing kind drag target");
            await page.mouse.move(from.x + from.width / 2, from.y + from.height / 2);
            await page.mouse.down();
            await page.mouse.move(to.x + to.width / 2, step.target.startsWith("lane:") ? to.y + to.height - 4 : to.y + to.height / 2);
            await page.mouse.up();
            break;
        }
        case "expectEmptyLauncher":
            await page.getByRole("button", { name: "Create workspace", exact: true }).waitFor();
            if (!(await page.getByRole("button", { name: "Create workspace", exact: true }).isDisabled())) throw new Error("Empty inventory permits workspace creation");
            if (await page.getByRole("alert").count()) throw new Error("Empty context inventory is reported as a failure");
            break;
        case "expectEntryCount": {
            const panel = page.getByRole("heading", { name: step.panel, exact: true }).locator("xpath=ancestor::section[1]");
            await panel.waitFor();
            await page.waitForFunction(({ panel, count }) => panel?.querySelectorAll("button").length === count, { panel: await panel.elementHandle(), count: step.count });
            break;
        }
        case "copyErrorDetail": {
            const titles = new Map([
                ["contexts", "Failed to load contexts"],
                ["namespaces", "Failed to load namespaces"],
                ["details", "KubeCove does not have permission for this"],
            ]);
            const alert = page.getByRole("alert").filter({ hasText: titles.get(step.error) });
            await alert.getByText("Technical detail", { exact: true }).click();
            await alert.getByText(step.expected, { exact: true }).waitFor();
            await alert.getByRole("button", { name: "Copy detail", exact: true }).click();
            await alert.getByRole("button", { name: "Copied", exact: true }).waitFor();
            break;
        }
        case "toggleResourcePin": {
            const rows = page.getByRole("row").filter({ has: page.getByRole("button", { name: new RegExp(`^(?:Pin|Unpin) ${step.kind} ${step.name}$`) }) }).filter({ hasText: step.namespace });
            const row = step.context === undefined ? rows : rows.filter({ has: page.getByRole("cell", { name: step.context, exact: true }) });
            await row.getByRole("button", { name: new RegExp(`^(?:Pin|Unpin) ${step.kind} ${step.name}$`) }).click();
            break;
        }
        case "expectWorkspaceEntry": case "openWorkspaceEntry": {
            const panel = page.getByRole("heading", { name: step.panel, exact: true }).locator("xpath=ancestor::section[1]");
            const buttons = panel.getByRole("button");
            await buttons.nth(step.index).filter({ hasText: step.label }).waitFor();
            if (await buttons.count() !== step.count) throw new Error("Workspace entry count differs");
            if (step.action === "openWorkspaceEntry") await buttons.nth(step.index).click();
            break;
        }
        case "expectActivePage": {
            await page.getByRole("treeitem", { selected: true }).filter({ has: page.getByText(step.page, { exact: true }) }).waitFor();
            break;
        }
        case "expectGitOpsSelection":
            await page.locator('[data-gitops-selected="true"]').filter({ hasText: step.name }).first().waitFor();
            break;
        case "reorderKindKey":
            await page.locator(`[data-kind-drop="${step.kind}"]`).focus();
            await page.keyboard.press(step.key);
            break;
        case "resetKindOrder":
            await page.getByRole("button", { name: "Reset", exact: true }).click();
            break;
        case "expectRemoteHost":
            await page.getByText(`Remote host: ${step.url}${step.disconnected ? " (disconnected)" : ""}`, { exact: true }).waitFor();
            break;
        case "dropRemoteConnection":
            await page.evaluate("import('/src/lib/settings-integration-mocks.ts').then(m => m.dropRemoteConnectionMock())");
            break;
        case "expectRemoteHostTooltip":
            await page.getByTitle(step.value, { exact: true }).waitFor();
            break;
        case "expectNoRemoteHost":
            // Waits for removal to render, as the settings text check does.
            await page.waitForFunction(() => !document.body.textContent?.includes("Remote host:"));
            break;
        case "configureShellRead":
            await page.evaluate(`import('/src/lib/tauri-dev-mock-shell.ts').then(m => m.configureShellRead(${JSON.stringify(step.phase)}))`);
            if (step.phase !== "ready") await page.reload();            break;
        case "retryLauncher":
            await page.getByRole("button", { name: "Retry", exact: true }).click();
            break;
        case "expectLauncherRetry":
            await page.getByRole("button", { name: "Retry", exact: true }).waitFor();
            break;
        case "overviewOperation":
            await page.locator('[data-slot="card"]').filter({ has: page.getByText("Operations", { exact: true }) }).getByRole("button", { name: step.destination, exact: true }).click();
            break;
        case "overviewIncident":
            await page.getByRole("button", { name: new RegExp(`^${step.filter} \\d+$`) }).click();
            break;
        case "reviewIncidentAction":
            await page.locator("[data-incident-guide]").getByRole("button", { name: "Review in Actions", exact: true }).nth(step.index).click();
            break;
        case "expectDetailTab":
            await page.getByRole("tab", { name: step.tab, exact: true, selected: true }).waitFor();
            break;
        default: throw new Error(`Not a shell action: ${step.action}`);
    }
}
