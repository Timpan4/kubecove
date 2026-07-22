import { $, browser, expect } from "@wdio/globals";
import { describe, it } from "mocha";

describe("native desktop smoke", () => {
	it("launches the isolated app and opens settings", async () => {
		await expect(browser).toHaveTitle(expect.stringContaining("KubeCove"));
		const contexts = await browser.execute(async () => {
			const tauri = (window as unknown as { __TAURI__: { core: { invoke: (name: string) => Promise<unknown[]> } } }).__TAURI__;
			return await tauri.core.invoke("list_kube_contexts");
		});
		expect(contexts).toHaveLength(2);
		const settings = await $('button[aria-label="Open settings"]');
		await settings.waitForClickable();
		await settings.click();
		await expect($("body")).toHaveText(expect.stringContaining("Settings"));
	});
});
