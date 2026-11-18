import { sharedActions, type WebActions } from "./web-action";

/** Latency report view timings and the interaction recorder's controls. */
export const diagnosticsActions = {
	async expectViewTiming(page, step) {
		// The web times each view's data request as `<view>.fetch.done`; GPUI times `<view>.present`.
		const row = page.getByRole("row").filter({ has: page.getByRole("cell", { name: `${step.view}.fetch.done`, exact: true }) });
		await row.waitFor();
		if (!(await row.textContent())?.includes("ms")) throw new Error("Missing measured duration");
	},
	...sharedActions(["copyInteractions", "clearInteractions"], async (page, step) => {
		if (step.action === "copyInteractions") {
			await page.evaluate(() => {
				Object.defineProperty(navigator, "clipboard", { configurable: true, value: {
					writeText: async (value: string) => { document.documentElement.dataset.interactionCopy = value; },
				} });
			});
		}
		await page.getByText("Interaction report", { exact: true }).locator("xpath=../..")
			.getByRole("button", { name: step.action === "copyInteractions" ? "Copy" : "Clear", exact: true }).click();
	}),
	async expectInteractionCopy(page, step) {
		const copied = await (await page.waitForFunction(() => document.documentElement.dataset.interactionCopy ?? "")).jsonValue();
		if (step.contains !== undefined && !copied.includes(step.contains)) throw new Error(`Interaction copy lacks ${step.contains}`);
		if (step.excludes !== undefined && copied.includes(step.excludes)) throw new Error(`Interaction copy includes ${step.excludes}`);
		if (step.succeededCommand !== undefined) {
			const succeeded = copied.split("\n").filter((line) => line.trim() !== "").some((line) => {
				const commands: { cmd: string; ok: boolean }[] = JSON.parse(line).commands ?? [];
				return commands.some((command) => command.cmd === step.succeededCommand && command.ok);
			});
			if (!succeeded) throw new Error(`Interaction copy lists no successful ${step.succeededCommand}`);
		}
	},
	async typeSettingsField(page, step) {
		const field = page.getByLabel(step.field, { exact: true });
		await field.click();
		await field.pressSequentially(step.value);
	},
} satisfies Partial<WebActions>;
