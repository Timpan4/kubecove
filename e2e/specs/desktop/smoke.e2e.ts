import { $, browser, expect } from "@wdio/globals";
import { describe, it } from "mocha";
import type { ClusterContext } from "../../../src/lib/types";

type TauriBridge = {
	core: {
		invoke: <Result, Payload = undefined>(name: string, payload?: Payload) => Promise<Result>;
		Channel: new <Message>() => { onmessage: (message: Message) => void };
	};
};

declare global {
	interface Window {
		__TAURI__: TauriBridge;
	}
}

async function listKubeContexts(): Promise<ClusterContext[]> {
	return browser.execute(() => window.__TAURI__.core.invoke<ClusterContext[]>("list_kube_contexts"));
}

describe("native desktop smoke", () => {
	it("launches the isolated app and opens settings", async () => {
		await expect(browser).toHaveTitle(expect.stringContaining("KubeCove"));
		const contexts = await listKubeContexts();
		expect(contexts).toHaveLength(2);
		const settings = await $('button[aria-label="Open settings"]');
		await settings.waitForClickable();
		await settings.click();
		await expect($("body")).toHaveText(expect.stringContaining("Settings"));
	});
});
