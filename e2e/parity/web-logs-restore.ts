import type { WebActions } from "./web-action";
import { expectLogToolbarFit } from "./web-logs-toolbar";
import type { Page } from "playwright-core";
import { tabTo } from "./web-keyboard";

// One-line log rows are `[data-log-row]` buttons. Their headline is an exact text node.
function logRow(page: Page, line: string) {
    return page.locator('[data-log-row][role="button"]').filter({ has: page.getByText(line, { exact: true }) });
}

function restorePrompt(page: Page) {
    return page.getByRole("alert").filter({ hasText: "Saved port forwards available" });
}

export const logRowAndRestoreActions = {
	async toggleLogRow(page, step) {
		if (step.key) {
			await tabTo(page, logRow(page, step.line));
			await page.keyboard.press(step.key);
		} else await logRow(page, step.line).getByText(step.line, { exact: true }).click();
	},
	async toggleLogExpandAll(page) {
		await page.getByRole("button", { name: /^(Expand|Collapse) all$/ }).click();
	},
	async expectLogRowOpen(page, step) {
		const row = logRow(page, step.line);
		await row.waitFor();
		if ((await row.getAttribute("aria-expanded")) !== String(step.open)) throw new Error(`Log row open state differs: ${step.line}`);
	},
	async copyLogLine(page, step) {
		await page.evaluate(() => Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText: async (value: string) => { document.documentElement.dataset.logCopy = value; } } }));
		await logRow(page, step.line).getByRole("button", { name: "Copy line", exact: true }).click();
		if (!(await page.locator("html").getAttribute("data-log-copy"))?.includes(step.contains)) throw new Error("Wrong log clipboard value");
	},
	async expectLogTimestampTooltip(page, step) {
		// Every layout keeps the time in a `<time>` inside its `[data-log-row]`, titled with the exact timestamp.
		const row = page.locator("[data-log-row]").filter({ has: page.getByText(step.line, { exact: true }) });
		await row.waitFor();
		const title = await row.locator("time").first().getAttribute("title");
		if (title !== step.value) throw new Error(`Log timestamp tooltip was ${title ?? "missing"}`);
	},
	async expectRestorePrompt(page, step) {
		const prompt = restorePrompt(page);
		if (!step.visible) {
			await prompt.waitFor({ state: "hidden" });
			return;
		}
		await prompt.waitFor();
		if (step.count !== undefined) {
			const text = `This workspace has ${step.count} saved Service ${step.count === 1 ? "forward" : "forwards"} ready to start.`;
			await prompt.getByText(text, { exact: true }).waitFor();
		}
	},
	async restorePromptControl(page, step) {
		const prompt = restorePrompt(page);
		if (step.control === "Auto-start") await prompt.getByRole("checkbox", { name: "Auto-start saved port forwards", exact: true }).click();
		else await prompt.getByRole("button", { name: step.control, exact: true }).click();
	},
	async expectLogLines(page, step) {
		// Headlines of the rendered rows, in display order.
		await page.waitForFunction((lines) => JSON.stringify([...document.querySelectorAll("[data-log-row]")].map((row) => row.querySelector('span[class*="text-[12.5px]"]')?.textContent?.trim())) === JSON.stringify(lines), step.lines);
	},
	async expectLogToolbarFit(page, step) {
		await expectLogToolbarFit(page, step.state);
	},
	async filterLogs(page, step) {
		await page.getByRole("textbox", { name: "Filter log lines" }).fill(step.value);
	},
	async toggleLogLevel(page, step) {
		await page.getByRole("group", { name: "Filter by level" }).getByRole("button", { name: new RegExp(`^${step.level} `) }).click();
	},
	async expectLogsEmpty(page, step) {
		await page.getByText(step.title, { exact: true }).waitFor();
	},
} satisfies Partial<WebActions>;
