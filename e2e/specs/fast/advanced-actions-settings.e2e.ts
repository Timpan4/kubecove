import { $, browser, expect } from "@wdio/globals";
import { beforeEach, describe, it } from "mocha";

describe("Advanced Actions settings", () => {
	beforeEach(async () => {
		await browser.url("/");
		await browser.execute(() => localStorage.clear());
		await browser.refresh();
		await $('button[aria-label="Open settings"]').click();
		await $('nav[aria-label="Settings sections"]').waitForDisplayed();
	});

	it("requires a second action, persists the choice, and disables immediately", async () => {
		await $('input[aria-label="Search settings"]').setValue("Skip change review");
		const setting = $('button[aria-label="Skip change review"]');
		await setting.waitForDisplayed();
		await expect(setting).toHaveAttribute("aria-checked", "false");

		await setting.click();
		await expect($('[role="alert"]')).toBeDisplayed();
		await expect($('[role="alert"]')).toHaveText(
			expect.stringContaining("A mistaken value may go live immediately"),
		);
		await $("button=Keep review").click();
		await expect(setting).toHaveAttribute("aria-checked", "false");
		await expect($('[role="alert"]')).not.toBeDisplayed();

		await setting.click();
		await $("button=Skip review").click();
		await expect(setting).toHaveAttribute("aria-checked", "true");
		await expect($('[role="alert"]')).not.toBeDisplayed();
		const persisted = await browser.execute(() =>
			JSON.parse(localStorage.getItem("kubecove-settings") ?? "{}").state.skipChangeReview,
		);
		expect(persisted).toBe(true);

		await browser.refresh();
		const searchSettings = $('input[aria-label="Search settings"]');
		await searchSettings.waitForDisplayed();
		await searchSettings.setValue("Skip change review");
		const enabledSetting = $('button[aria-label="Skip change review"]');
		await expect(enabledSetting).toHaveAttribute("aria-checked", "true");
		await enabledSetting.click();
		await expect(enabledSetting).toHaveAttribute("aria-checked", "false");
		await expect($('[role="alert"]')).not.toBeDisplayed();
	});
});
