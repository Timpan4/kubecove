import type { WebActions } from "./web-action";
import { waitFor } from "../../.agents/skills/verify-kubecove/scripts/processes";

export const clipboardRecorder = `Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText: async (value) => { document.documentElement.dataset.copyValue = value; } } })`;

export const feedbackActions = {
	async expectForegroundLoading(page, step) {
		let expected = `the foreground loading bar to be ${step.active ? "active" : "idle"}`;
		if (step.sliding !== undefined) expected += step.sliding ? " and sliding" : " and still";
		// Polled from this process: a scenario that pauses the page clock also pauses the page's own timers.
		await waitFor(expected, () => page.evaluate(([active, sliding]) => {
			const host = document.querySelector("[data-foreground-loading]");
			if (host?.getAttribute("data-active") !== active) return false;
			if (sliding === null) return true;
			// The keyframe animation moves the bar while it runs. Reduced motion and the end of the fade-out both remove it.
			const bar = host.firstElementChild;
			return bar !== null && bar.getAnimations().some((animation) => animation.playState === "running") === sliding;
		}, [step.active ? "true" : "false", step.sliding ?? null] as const));
	},
	async expectMotion(page, step) {
		// CSS animations run on the browser's clock, not the page clock the scenario controls, so a finite one ends by itself.
		await waitFor(step.moving ? "a running animation" : "every animation to end", () => page.evaluate((moving) => document.getAnimations().some((animation) => animation.playState === "running") === moving, step.moving));
	},
	async setReducedMotion(page, step) {
		await page.emulateMedia({ reducedMotion: step.reduce ? "reduce" : "no-preference" });
	},
	async expectFriendlyError(page, step) {
		const alert = page.getByRole("alert").filter({ hasText: step.title });
		await alert.getByText(step.title, { exact: true }).waitFor();
		if (step.next !== undefined) await alert.getByText(step.next, { exact: true }).waitFor();
		if (step.summary !== undefined) await alert.getByText(step.summary, { exact: true }).waitFor();
	},
	async expectCredentialFailure(page, step) {
		const alert = page.getByRole("alert").filter({ hasText: step.title });
		await alert.getByText(step.title, { exact: true }).waitFor();
		await alert.getByText(step.summary, { exact: true }).waitFor();
		await alert.getByLabel("Plugin output", { exact: true }).getByText(step.pluginMessage, { exact: true }).waitFor();
		await alert.getByLabel("Sign-in details", { exact: true }).getByText(step.command, { exact: true }).waitFor();
	},
	async copyCredentialCommand(page, step) {
		await page.evaluate(clipboardRecorder);
		// An earlier copy of the same command must not satisfy this one.
		await page.evaluate(() => delete document.documentElement.dataset.copyValue);
		const alert = page.getByRole("alert").filter({ hasText: step.title });
		await alert.getByRole("button", { name: "Copy terminal command", exact: true }).click();
		await alert.getByRole("button", { name: "Copied", exact: true }).waitFor();
		const copied = await page.evaluate(() => document.documentElement.dataset.copyValue);
		if (copied !== step.expected) throw new Error(`Copied ${JSON.stringify(copied)}, expected ${JSON.stringify(step.expected)}`);
	},
	async inspectRequiredPermission(page) {
		await page.getByRole("alert").getByRole("button", { name: "Inspect required permission", exact: true }).click();
	},
	async expectPermissionTarget(page, step) {
		for (const [name, expected] of [["Verb", step.verb], ["Resource", step.resource]] as const) {
			const field = page.getByRole("textbox", { name, exact: true });
			await field.waitFor();
			const value = await field.inputValue();
			if (value !== expected) throw new Error(`RBAC ${name} is ${JSON.stringify(value)}, expected ${JSON.stringify(expected)}`);
		}
	},
	async returnToSource(page) {
		await page.getByRole("button", { name: /^Return to / }).click();
	},
	async copyResourceName(page, step) {
		const copyName = `Copy resource name: ${step.name}`;
		await page.evaluate(clipboardRecorder);
		await page.getByRole("row").filter({ hasText: step.namespace })
			.filter({ has: page.getByText(step.kind, { exact: true }) })
			.filter({ has: page.getByRole("button", { name: copyName, exact: true }) })
			.getByRole("button", { name: copyName, exact: true })
			.click();
		await page.waitForFunction(() => document.documentElement.dataset.copyValue !== undefined);
		const copied = await page.evaluate(() => document.documentElement.dataset.copyValue);
		if (copied !== step.name) throw new Error(`Copied ${JSON.stringify(copied)}, expected ${JSON.stringify(step.name)}`);
	},
	async expectCopyToast(page, step) {
		await page.getByRole("status").getByText(step.text, { exact: true }).waitFor();
	},
	async dismissCopyToast(page) {
		await page.getByRole("button", { name: "Dismiss copy notification", exact: true }).click();
	},
} satisfies Partial<WebActions>;
