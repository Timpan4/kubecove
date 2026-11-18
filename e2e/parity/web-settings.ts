import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import type { WebActions } from "./web-action";
import type { Page } from "playwright-core";
export const settingsActions = {
	async fillSettingsField(page, step) {
		await page.getByLabel(step.field, { exact: true }).fill(step.value);
	},
	async clickSettingsButton(page, step) {
		await page.getByRole("button", { name: step.label, exact: true }).click();
	},
	async expectSettingsText(page, step) {
		await page.getByText(step.value, { exact: false }).first().waitFor();
	},
	async expectNoSettingsText(page, step) {
		// Waits for removal to render; text that never rendered passes on the first poll.
		await page.waitForFunction((value) => !document.body.textContent?.includes(value), step.value);
	},
	async expectMaskedToken(page) {
		if (await page.getByLabel("Server token", { exact: true }).getAttribute("type") !== "password") throw new Error("Credential is not masked");
	},
	async expectRuntimeEndpoint(page, step) {
		await loadSettingsMocks(page);
		await page.waitForFunction((expected) => (window.paritySettingsMocks.remoteMockStatus()?.serverUrl ?? "local") === expected, step.value);
	},
	async reopenStoredSettings(page) {
		{
			// A fresh checkout, such as a CI runner, has no ignored `.t` directory yet.
			const parent = process.env.TMPDIR ?? join(process.cwd(), ".t");
			await mkdir(parent, { recursive: true });
			const directory = await mkdtemp(join(parent, "parity-settings-"));
			try {
				const snapshot = await page.evaluate("import('/src/lib/tauri-dev-mocks.ts').then(m => m.createDevMockTauriClient().invoke('get_kubeconfig_sources'))");
				const path = join(directory, "sources.json");
				await writeFile(path, JSON.stringify(snapshot));
				await page.getByRole("button", { name: "Back to app", exact: true }).click();
				// The app reads and caches kubeconfig sources at startup. Restoring them after the
				// reload raced that read and failed full-suite runs, so seed them before the app loads.
				const stored: unknown = JSON.parse(await readFile(path, "utf8"));
				await page.addInitScript((sources) => { Object.assign(globalThis, { __kubecoveParityKubeconfigSources: sources }); }, stored);
				await page.reload();
				await page.getByRole("button", { name: "Open settings", exact: true }).click();
			} finally { await rm(directory, { recursive: true }); }
		}
	},
	async configureSettingsMock(page, step) {
		await page.evaluate(`import('/src/lib/settings-integration-mocks.ts').then(m => m.configureSettingsMock(${JSON.stringify(step.script)}))`);
	},
	async openSettings(page) {
		await page.getByRole("button", { name: "Open settings", exact: true }).click();
	},
	async closeSettings(page) {
		await page.getByRole("button", { name: "Back to app", exact: true }).click();
	},
	async showSettingsSection(page, step) {
		await page.getByRole("navigation", { name: "Settings sections" }).getByRole("button", { name: step.section, exact: true }).click();
	},
	async searchSettings(page, step) {
		await page.getByRole("textbox", { name: "Search settings", exact: true }).fill(step.value);
	},
	async expectSettingsHeading(page, step) {
		await page.getByRole("heading", { level: 2, name: step.heading, exact: true }).waitFor();
	},
	async expectSettingRow(page, step) {
		await page.getByText(step.title, { exact: true }).first().waitFor();
	},
	async expectNoSettingRow(page, step) {
		if (await page.getByText(step.title, { exact: true }).count()) throw new Error(`${step.title} is shown`);
	},
	async expectSettingsEmpty(page, step) {
		await page.getByText(`No settings match "${step.value}".`, { exact: true }).waitFor();
	},
	async setKubeconfigEnvVar(page, step) {
		await page.getByRole("textbox", { name: "Kubeconfig env var", exact: true }).fill(step.value);
		const save = page.getByRole("button", { name: "Save", exact: true });
		await save.click();
		await save.and(page.locator(":enabled")).waitFor();
	},
	async expectKubeconfigEnvVar(page, step) {
		const input = await page.getByRole("textbox", { name: "Kubeconfig env var", exact: true }).elementHandle();
		await page.waitForFunction(({ input, value }) => input instanceof HTMLInputElement && input.value === value, { input, value: step.value });
	},
	async selectSettingOption(page, step) {
		await page.getByText(step.setting, { exact: true }).locator("xpath=../..").getByRole("radio", { name: step.value, exact: true }).click();
	},
	async expectSettingOption(page, step) {
		const button = page.getByText(step.setting, { exact: true }).locator("xpath=../..").getByRole("radio", { name: step.value, exact: true });
		if (await button.getAttribute("aria-checked") !== "true") throw new Error("Setting option not selected");
	},
	async moveResourceKindEarlier(page, step) {
		await page.getByRole("button", { name: `${step.kind}, Show first`, exact: true }).press("ArrowUp");
	},
	async expectResourceKindPosition(page, step) {
		const item = page.getByRole("region", { name: step.lane, exact: true }).getByRole("button").nth(step.index);
		if (await item.getAttribute("aria-label") !== `${step.kind}, ${step.lane}`) throw new Error("Resource kind order changed");
	},
	async restartClient(page) {
		await page.reload(); await page.getByRole("button", { name: "Open settings", exact: true }).waitFor();
	},
	async toggleSetting(page, step) {
		await page.getByRole("switch", { name: step.setting, exact: true }).click();
	},
	async expectSetting(page, step) {
		await page.getByRole("switch", { name: step.setting, exact: true }).and(page.locator(`[aria-checked="${step.checked}"]`)).waitFor();
	},
} satisfies Partial<WebActions>;

export async function loadSettingsMocks(page: Page) {
	await page.evaluate("import('/src/lib/settings-integration-mocks.ts').then(m => { globalThis.paritySettingsMocks = m; })");
}
