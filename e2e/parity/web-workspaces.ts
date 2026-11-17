import type { WebActions } from "./web-action";
import { isDeepStrictEqual } from "node:util";
import original from "../../src/lib/settings-integration-workspace.json";
import { loadSettingsMocks } from "./web-settings";

export function expectRetainedWorkspaceFields(items: { spec: { displayName: string; scope: typeof original.spec.scope; portForwards: unknown } }[], name: string) {
    const item = items.find((item) => item.spec.displayName === name);
    if (!item) throw new Error("Missing exported workspace");
    for (const field of ["kinds", "layout", "gitOpsFilter", "argoAppFilter", "shortcutPreferences"] as const) {
        if (!isDeepStrictEqual(item.spec.scope[field], original.spec.scope[field])) throw new Error(`Workspace lost ${field}`);
    }
    if (!isDeepStrictEqual(item.spec.portForwards, original.spec.portForwards)) throw new Error("Workspace lost saved forwards");
}

export const workspaceActions = {
	async failWorkspaceContext(page, step) {
		await page.evaluate(`import('/src/lib/tauri-dev-mocks.ts').then(m => m.failWorkspaceContext(${JSON.stringify(step.context)}))`);
	},
	async holdWorkspaceContext(page, step) {
		await page.evaluate(`import('/src/lib/tauri-dev-mocks.ts').then(m => m.holdWorkspaceContext(${JSON.stringify(step.context)}))`);
	},
	async releaseWorkspaceContext(page, step) {
		await page.evaluate(`import('/src/lib/tauri-dev-mocks.ts').then(m => m.releaseWorkspaceContext(${JSON.stringify(step.context)}))`);
	},
	async expectContextsLoading(page, step) {
		const notice = page.locator("[data-contexts-loading]");
		if (!step.contexts.length) {
			await notice.waitFor({ state: "hidden" });
			return;
		}
		const names = step.contexts.map((context) => `"${context}"`).join(", ");
		const text = `Waiting for context${step.contexts.length > 1 ? "s" : ""} ${names}. Showing resources from the contexts that answered.`;
		await notice.filter({ hasText: text }).waitFor();
	},
	async expectWorkspaceReadWarning(page, step) {
		await page.getByRole("alert").filter({ hasText: step.context }).filter({ hasText: "Context unavailable" }).waitFor();
	},
	async expectContextUnavailable(page, step) {
		await page.getByRole("group", { name: `${step.name} actions`, exact: true }).locator('xpath=ancestor::*[@data-slot="card"]').getByText("Context unavailable", { exact: true }).waitFor();
	},
	async editSavedWorkspace(page, step) {
		await page.getByRole("button", { name: `Edit ${step.name}`, exact: true }).click();
	},
	async expectNoSavedWorkspace(page, step) {
		await page.getByRole("button", { name: `Open ${step.name}`, exact: true }).waitFor({ state: "hidden" });
	},
	async saveWorkspace(page) {
		await page.getByRole("button", { name: "Save workspace", exact: true }).click();
		await page.getByText("New workspace", { exact: true }).waitFor();
	},
	async cancelWorkspaceEdit(page) {
		await page.getByRole("button", { name: "Cancel edit", exact: true }).click();
	},
	async toggleWorkspaceContext(page, step) {
		await page.getByRole("checkbox", { name: step.context, exact: true }).click();
	},
	async chooseClusterContext(page, step) {
		await page.locator("#cluster-select").click();
		await page.getByRole("option", { name: step.context, exact: true }).click();
	},
	async expectOverviewHealth(page, step) {
		await page.locator("[data-overview-total]").getByText(`${step.total} resource${step.total === 1 ? "" : "s"} loaded from the saved scope.`, { exact: true }).waitFor();
	},
	async exportSavedWorkspace(page, step) {
		await page.getByRole("button", { name: `Export ${step.name}`, exact: true }).click();
		await page.getByText(`Exported ${step.name}.`, { exact: true }).waitFor();
	},
	async expectExportedShortcut(page, step) {
		const content = String(await page.evaluate("import('/src/lib/settings-integration-mocks.ts').then(m => m.workspaceExportMock())"));
		const document = JSON.parse(content);
		const items: { spec: { displayName: string; shortcuts: unknown[] } }[] = document.kind === "WorkspaceList" ? document.items : [document];
		const shortcuts = items.find((item) => item.spec.displayName === step.name)?.spec.shortcuts ?? [];
		if (!shortcuts.some((shortcut) => isDeepStrictEqual(shortcut, step.shortcut))) throw new Error(`Export of ${step.name} lacks shortcut ${JSON.stringify(step.shortcut)}: ${JSON.stringify(shortcuts)}`);
	},
	// The detail header names the target context: a badge with several contexts, screen-reader text with one.
	async expectDetailContext(page, step) {
		await page.getByRole("button", { name: "Close resource details", exact: true }).locator("xpath=ancestor::header")
		.getByText(`Context: ${step.context}`, { exact: true }).waitFor();
	},
	async expectWorkspaceExport(page, step) {
		await loadSettingsMocks(page);
		const content = await (await page.waitForFunction(() => window.paritySettingsMocks.workspaceExportMock())).jsonValue();
		const document = JSON.parse(content);
		const items = document.kind === "WorkspaceList" ? document.items : [document];
		if (!items.some((item: { spec: { displayName: string; scope: { clusterContext: string; namespaces: string[] } } }) => item.spec.displayName === step.name && item.spec.scope.clusterContext === "mock-dev" && item.spec.scope.namespaces.includes("payments"))) throw new Error("Export ignored the imported workspace scope");
		expectRetainedWorkspaceFields(items, step.name);
	},
	async openSavedWorkspace(page, step) {
		await page.getByRole("button", { name: `Open ${step.name}`, exact: true }).click(); await page.waitForURL(/#\/workspace\/[^/]+\//);
	},
	async expectSavedWorkspace(page, step) {
		await page.getByRole("group", { name: `${step.name} actions`, exact: true }).waitFor();
		await page.getByRole("group", { name: `${step.name} actions`, exact: true }).locator('xpath=ancestor::*[@data-slot="card"]').getByText(step.summary, { exact: true }).waitFor();
	},
	async nameWorkspace(page, step) {
		await page.locator("#workspace-name").fill(step.value);
	},
	async selectWorkspaceContext(page, step) {
		await page.getByRole("combobox").click();
		await page.getByRole("option", { name: step.context, exact: true }).click();
	},
	async toggleWorkspaceNamespace(page, step) {
		await page.getByRole("checkbox", { name: step.namespace, exact: true }).click();
	},
	async createWorkspace(page) {
		await page.getByRole("button", { name: "Create workspace", exact: true }).click(); await page.waitForURL(/#\/workspace\/[^/]+\/overview$/);
	},
	async expectOpenWorkspace(page, step) {
		await page.locator("div.min-w-0").filter({ has: page.getByText(step.name, { exact: true }) }).filter({ has: page.getByText(step.context, { exact: true }) }).first().waitFor();
	},
} satisfies Partial<WebActions>;
