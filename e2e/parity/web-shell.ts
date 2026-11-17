import { sharedActions, type WebActions } from "./web-action";
import { loadDeferredPage } from "./web-deferred";
import type { Page } from "playwright-core";
import { STARTUP_HOLDS_KEY, type TimingControl } from "../../src/lib/tauri-dev-mock-timing";
import type { Scenario, Step } from "./contract";
import { tabTo } from "./web-keyboard";

const escapeRegExp = (value: string) => value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

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

export const shellActions = {
	async startWithoutWorkspace(page) {
		await page.getByRole("button", { name: "Create workspace", exact: true }).waitFor();
	},
	async expectNoWorkspaceShell(page) {
		await page.getByRole("button", { name: "Search views, namespaces, and resources", exact: true }).waitFor({ state: "detached" });
		if (await page.getByRole("tree").count()) throw new Error("Sidebar rendered without an open workspace");
	},
	async expectPaletteUnavailable(page) {
		await page.keyboard.press("ControlOrMeta+K");
		if (await page.getByRole("dialog").count()) throw new Error("Palette opened without an open workspace");
	},
	async openLauncher(page) {
		// Wait for either view: a launcher that renders after a one-off visibility check is still the launcher.
		const launcher = page.getByRole("button", { name: "Create workspace", exact: true });
		const open = page.getByRole("button", { name: "Open workspaces", exact: true });
		await launcher.or(open).first().waitFor();
		if (!await launcher.isVisible()) await open.click();
	},
	...sharedActions(["expectOverviewShortcuts", "overviewShortcut"], async (page, step) => {
		const section = page.getByRole("heading", { name: "Shortcuts", exact: true }).locator("xpath=ancestor::section[1]");
		if (step.action === "overviewShortcut") await section.getByRole("button", { name: step.label, exact: true }).click();
		else {
			const labels = await section.getByRole("button").allTextContents();
			if (JSON.stringify(labels.map(label => label.trim())) !== JSON.stringify(step.labels)) throw new Error("Saved shortcuts differ");
		}
	}),
	async expectOverviewScope(page, step) {
		for (const label of step.labels) await page.locator("section > header").getByText(label, { exact: true }).waitFor();
	},
	async expectCompareTotal(page, step) {
		const comparison = page.getByText(step.title, { exact: true }).locator("..");
		const side = comparison.getByText(step.side, { exact: true }).locator("..");
		await side.getByText(new RegExp(`^${step.total} total /`)).waitFor();
	},
	async retryOverview(page) {
		await page.getByRole("button", { name: "Retry workspace resources", exact: true }).click();
	},
	async expectOverviewRetry(page) {
		await page.getByRole("button", { name: "Retry workspace resources", exact: true }).waitFor();
	},
	async dragKind(page, step) {
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
	},
	async expectEmptyLauncher(page) {
		await page.getByRole("button", { name: "Create workspace", exact: true }).waitFor();
		if (!(await page.getByRole("button", { name: "Create workspace", exact: true }).isDisabled())) throw new Error("Empty inventory permits workspace creation");
		if (await page.getByRole("alert").count()) throw new Error("Empty context inventory is reported as a failure");
	},
	async expectEntryCount(page, step) {
		const panel = page.getByRole("heading", { name: step.panel, exact: true }).locator("xpath=ancestor::section[1]");
		await panel.waitFor();
		await page.waitForFunction(({ panel, count }) => panel?.querySelectorAll("button").length === count, { panel: await panel.elementHandle(), count: step.count });
	},
	async copyErrorDetail(page, step) {
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
	},
	async toggleResourcePin(page, step) {
		const pin = new RegExp(`^(?:Pin|Unpin) ${escapeRegExp(step.kind)} ${escapeRegExp(step.name)}$`);
		const rows = page.getByRole("row").filter({ has: page.getByRole("button", { name: pin }) }).filter({ hasText: step.namespace });
		const row = step.context === undefined ? rows : rows.filter({ has: page.getByRole("cell", { name: step.context, exact: true }) });
		const button = row.getByRole("button", { name: pin });
		if (step.key) {
			await tabTo(page, button);
			await page.keyboard.press(step.key);
		} else await button.click();
	},
	...sharedActions(["expectWorkspaceEntry", "openWorkspaceEntry"], async (page, step) => {
		const panel = page.getByRole("heading", { name: step.panel, exact: true }).locator("xpath=ancestor::section[1]");
		const buttons = panel.getByRole("button");
		const button = buttons.nth(step.index);
		await button.waitFor();
		const kind = step.label.split(" / ")[0];
		const subtitle = await button.locator("span.font-normal").innerText();
		const accessibleName = `${step.label}${subtitle.slice(kind.length)}`;
		await button.and(panel.getByRole("button", { name: accessibleName, exact: true })).waitFor();
		if (await buttons.count() !== step.count) throw new Error("Workspace entry count differs");
		if (step.action === "openWorkspaceEntry") await buttons.nth(step.index).click();
	}),
	async expectActivePage(page, step) {
		await page.getByRole("treeitem", { selected: true }).filter({ has: page.getByText(step.page, { exact: true }) }).waitFor();
	},
	async expectGitOpsSelection(page, step) {
		const selected = page.locator('[data-gitops-selected="true"]').filter({ hasText: step.name });
		// A namespace tells same-named Applications apart by the selected card's Details button.
		const card = step.namespace === undefined ? selected : selected.filter({
			has: page.getByRole("button", { name: `Open details for Application ${step.name} in ${step.namespace}`, exact: true }),
		});
		await card.first().waitFor();
	},
	async reorderKindKey(page, step) {
		await tabTo(page, page.locator(`[data-kind-drop="${step.kind}"]`));
		await page.keyboard.press(step.key);
	},
	async resetKindOrder(page) {
		await page.getByRole("button", { name: "Reset", exact: true }).click();
	},
	async expectRemoteHost(page, step) {
		await page.getByText(`Remote host: ${step.url}${step.disconnected ? " (disconnected)" : ""}`, { exact: true }).waitFor();
	},
	async dropRemoteConnection(page) {
		await page.evaluate("import('/src/lib/settings-integration-mocks.ts').then(m => m.dropRemoteConnectionMock())");
	},
	async expectRemoteHostTooltip(page, step) {
		await page.getByTitle(step.value, { exact: true }).waitFor();
	},
	async expectNoRemoteHost(page) {
		// Waits for removal to render, as the settings text check does.
		await page.waitForFunction(() => !document.body.textContent?.includes("Remote host:"));
	},
	async configureShellRead(page, step) {
		await page.evaluate(`import('/src/lib/tauri-dev-mock-shell.ts').then(m => m.configureShellRead(${JSON.stringify(step.phase)}))`);
		if (step.phase !== "ready") await page.reload();
	},
	async retryLauncher(page) {
		await page.getByRole("button", { name: "Retry", exact: true }).click();
	},
	async expectLauncherRetry(page, step) {
		const retry = page.getByRole("button", { name: "Retry", exact: true });
		await retry.waitFor();
		if (step.height !== undefined) {
			const bounds = await retry.boundingBox();
			if (!bounds) throw new Error("Retry has no bounds");
			if (Math.abs(bounds.height - step.height) >= 0.5) throw new Error(`Retry is ${bounds.height}px high`);
		}
	},
	async overviewOperation(page, step) {
		if (step.destination === "Resources") await page.getByRole("button", { name: "Open resources", exact: true }).click();
		else if (step.destination === "Workspaces") await page.getByRole("button", { name: "Open workspaces", exact: true }).click();
		else if (step.destination === "Incidents") await page.getByRole("button", { name: "Open incident queue", exact: true }).click();
		else await page.getByRole("treeitem", { name: "Port Forwards", exact: true }).click();
	},
	async reviewIncidentAction(page, step) {
		await page.locator("[data-incident-guide]").getByRole("button", { name: "Review in Actions", exact: true }).nth(step.index).click();
	},
	async expectDetailTab(page, step) {
		await page.getByRole("tab", { name: step.tab, exact: true, selected: true }).waitFor();
	},
	async openPage(page, step) {
		await openPage(page, step.page);
	},
	// GitOps rows render above the namespace sections, so a shared label such as Workloads matches the GitOps node first.
	async expandNav(page, step) {
		await page.getByRole("button", { name: `Expand ${step.page}`, exact: true }).first().click();
	},
} satisfies Partial<WebActions>;

export async function openPage(page: Page, name: string) {
	// Rows with children also name their expand toggle.
	const label = literal(name);
	await loadDeferredPage(page, name);
	await page.getByRole("treeitem", { name: new RegExp(`^(?:(?:Expand|Collapse) ${label} )?${label}$`) }).click();
}

function literal(text: string) { return text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"); }
