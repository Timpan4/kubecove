import { writeFile } from "node:fs/promises";
import { join } from "node:path";
import { browser, expect } from "@wdio/globals";
import { describe, it } from "mocha";
import type { BackendCacheDiagnosticSnapshot } from "../../../src/lib/diagnostics-types";

describe("native cache diagnostics", () => {
	it("keeps opt-in snapshots separate from timing history and displays the cache report", async () => {
		const artifacts = process.env.KUBECOVE_E2E_ARTIFACTS;
		if (!artifacts) throw new Error("Profile artifact directory is required");
		await writeFile(join(artifacts, "backend-cache-diagnostics.json"), `${JSON.stringify({ passed: false, stage: "started" })}\n`);
		await browser.waitUntil(async () => browser.execute(() =>
			performance.getEntriesByName("kubecove:startup:launcher-ready").length > 0),
			{ timeout: 30_000, timeoutMsg: "Launcher did not become ready" });
		const observations = await browser.execute(async () => {
			const invoke = window.__TAURI__.core.invoke;
			await invoke("set_backend_diagnostics_enabled", { enabled: false });
			const disabled = await invoke<BackendCacheDiagnosticSnapshot[]>("get_backend_cache_diagnostics");
			await invoke("set_backend_diagnostics_enabled", { enabled: true });
			const historyBefore = await invoke("get_backend_diagnostics");
			const caches = await invoke<BackendCacheDiagnosticSnapshot[]>("get_backend_cache_diagnostics");
			await invoke("get_backend_cache_diagnostics");
			const historyAfter = await invoke("get_backend_diagnostics");
			await invoke("clear_backend_diagnostics");
			const cleared = await invoke<BackendCacheDiagnosticSnapshot[]>("get_backend_cache_diagnostics");
			return { disabled, caches, cleared, historyUnchanged: JSON.stringify(historyBefore) === JSON.stringify(historyAfter) };
		});
		expect(observations.disabled).toEqual([]);
		expect(observations.historyUnchanged).toBe(true);
		expect(observations.caches.map((cache) => cache.label)).toEqual([
			"namespaces", "resource_kinds", "present_custom_resource_kinds", "resources", "topologies", "flux_ownership_indexes",
		]);
		for (const cache of observations.cleared) {
			expect(Object.keys(cache).sort()).toEqual([
				"dirty", "evictions", "hits", "joins", "label", "loading", "misses", "ready", "restoredReloadFailures", "retainedItems", "shallowPayloadBytes", "weightKind",
			].sort());
			expect(cache.weightKind).toBe("shallow-payload-lower-bound");
			expect([cache.hits, cache.misses, cache.joins, cache.evictions, cache.restoredReloadFailures]).toEqual([0, 0, 0, 0, 0]);
		}
		const recordStage = async (stage: string) => writeFile(join(artifacts, "backend-cache-diagnostics.json"), `${JSON.stringify({ passed: false, stage, ...observations }, null, 2)}\n`);
		await recordStage("native-command-checks-passed");
		// Native driver element commands stall on this headless Wayland host.
		// DOM events exercise the app handlers; physical pointer input is not measured.
		await browser.execute(() => {
			const settings = document.querySelector<HTMLButtonElement>('button[aria-label="Open settings"]');
			if (!settings) throw new Error("Settings button is missing");
			settings.click();
		});
		await browser.waitUntil(async () => browser.execute(() => Boolean(document.querySelector('nav[aria-label="Settings sections"]'))));
		await recordStage("settings-open");
		await browser.execute(() => {
			const diagnostics = Array.from(document.querySelectorAll<HTMLButtonElement>('nav[aria-label="Settings sections"] button'))
				.find((button) => button.textContent?.trim() === "Diagnostics");
			if (!diagnostics) throw new Error("Diagnostics settings section is missing");
			diagnostics.click();
		});
		await recordStage("diagnostics-open");
		await browser.execute(() => {
			const toggle = document.querySelector<HTMLButtonElement>('[aria-label="Enable diagnostics"]');
			if (!toggle) throw new Error("Diagnostics toggle is missing");
			toggle.click();
		});
		await browser.waitUntil(async () => browser.execute(() => document.body.textContent?.includes("flux_ownership_indexes") === true));
		await recordStage("cache-table-visible");
		await browser.execute(() => {
			Array.from(document.querySelectorAll("div"))
				.find((element) => element.textContent === "Backend caches")?.scrollIntoView({ block: "center" });
		});
		await browser.saveScreenshot(join(artifacts, "backend-cache-diagnostics.png"));
		await writeFile(join(artifacts, "backend-cache-diagnostics.json"), `${JSON.stringify({ passed: true, scenario: "launcher-native-cache-contract", ...observations }, null, 2)}\n`);
	});
});
