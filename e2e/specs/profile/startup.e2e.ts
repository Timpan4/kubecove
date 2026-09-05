import { readFile, writeFile } from "node:fs/promises";
import { arch, release } from "node:os";
import { join } from "node:path";
import { browser, expect } from "@wdio/globals";
import { describe, it } from "mocha";
import type { AppUsageMetrics } from "../../../src/lib/types";
import { startupReport } from "../../harness/startup-report";

describe("release-shaped startup profile", () => {
	it("records an isolated launcher baseline", async () => {
		const artifacts = process.env.KUBECOVE_E2E_ARTIFACTS;
		if (!artifacts) throw new Error("Profile artifact directory is required");
		await browser.waitUntil(async () => browser.execute(() =>
			performance.getEntriesByName("kubecove:startup:launcher-ready").length > 0),
			{ timeout: 30_000, timeoutMsg: "Launcher did not become ready" });
		const frontend = await browser.execute(() => {
			// SAFETY: Chromium's optional memory extension is absent on WebKit.
			const memory = (performance as Performance & { memory?: { usedJSHeapSize: number } }).memory;
			return {
				collectedAtMs: performance.now(),
				entries: performance.getEntries().filter((entry) => entry.name.startsWith("kubecove:"))
					.map(({ name, startTime, duration }) => ({ name, startTime, duration })),
				heapBytes: memory?.usedJSHeapSize ?? null,
				viewport: { width: innerWidth, height: innerHeight, scale: devicePixelRatio },
			};
		});
		const usage = await browser.execute(() => window.__TAURI__.core.invoke<AppUsageMetrics>("get_app_usage_metrics"))
			.catch(() => null);
		const report = {
			schemaVersion: 1,
			scenario: "launcher-fresh-profile",
			build: "release-with-e2e-feature",
			frontendCollectedAtMs: frontend.collectedAtMs,
			limitations: ["E2E automation overhead is included", "OS filesystem caches are not flushed", "No live cluster; workspace and resource milestones are not exercised", "Process-launch-to-WebView-navigation time is unavailable"],
			environment: {
				platform: process.platform, architecture: arch(), osRelease: release(),
				webview: process.platform === "win32" ? "WebView2" : process.platform === "darwin" ? "WKWebView" : "WebKitGTK",
				sourceSha: process.env.KUBECOVE_PROFILE_SHA ?? null,
				sourceDirty: process.env.KUBECOVE_PROFILE_DIRTY === "true",
				...frontend.viewport,
			},
			...startupReport(frontend.entries, frontend.heapBytes, usage),
			bundle: JSON.parse(await readFile(join(artifacts, "frontend-bundle.json"), "utf8")),
		};
		await writeFile(join(artifacts, "startup.json"), `${JSON.stringify(report, null, 2)}\n`);
		for (const name of ["frontend-entry", "svelte-mount", "path-restored", "kubeconfig-ready", "launcher-ready"]) {
			expect(report.milestones.find((milestone) => milestone.name === name)?.atMs).not.toBeNull();
		}
		expect(report.ipc.commands.find((entry) => entry.command === "get_kubeconfig_sources")?.count).toBe(1);
		expect(report.ipc.limitReached).toBe(false);
	});
});
