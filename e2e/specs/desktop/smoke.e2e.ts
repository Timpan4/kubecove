import { $, browser, expect } from "@wdio/globals";
import { describe, it } from "mocha";
import type { AppUsageMetrics, ClusterContext } from "../../../src/lib/types";

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

	it("samples the memory figures this platform reports", async () => {
		const sample = () =>
			browser.execute(() =>
				window.__TAURI__.core.invoke<AppUsageMetrics, { measureWorkingSet: boolean }>("get_app_usage_metrics", {
					measureWorkingSet: true,
				}),
			);
		const first = await sample();
		const { memory, workingSetWindowMs } = await sample();
		if (process.platform === "win32") {
			expect(memory.wssBytes).toBeGreaterThan(0);
			expect(memory.rssBytes).toBeNull();
			expect(memory.pssBytes).toBeNull();
		} else if (process.platform === "darwin") {
			expect(memory.wssBytes).toBeNull();
			expect(memory.rssBytes).toBeGreaterThan(0);
			expect(memory.pssBytes).toBeNull();
		} else {
			// Linux WSS covers the interval since the previous sample reset referenced pages.
			expect(first.memory.wssBytes).toBeNull();
			expect(memory.wssBytes).toBeGreaterThan(0);
			expect(workingSetWindowMs).not.toBeNull();
			expect(memory.rssBytes).toBeGreaterThan(0);
			expect(memory.pssBytes).toBeGreaterThan(0);
		}
	});
});
