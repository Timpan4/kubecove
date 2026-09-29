import { writeFile } from "node:fs/promises";
import { join } from "node:path";
import { browser, expect } from "@wdio/globals";
import { describe, it } from "mocha";
import { LARGE_QUERY_GC_TIME_MS, LARGE_QUERY_ROOTS } from "../../../src/lib/query-retention";
import type { AppUsageMetrics } from "../../../src/lib/types";
import { startupReport } from "../../harness/startup-report";

const profile = process.env.KUBECOVE_MEMORY_FIXTURE ? describe : describe.skip;

profile("release-shaped frontend memory", () => {
	let refetchAfterCollection: (() => Promise<void>) | undefined;
	it("records payloads across inspection, workspace switching, and configured inactive collection", async () => {
		const artifacts = process.env.KUBECOVE_E2E_ARTIFACTS;
		if (!artifacts) throw new Error("Profile artifact directory is required");
		const stages: unknown[] = [];
		const report = { passed: false, fixture: process.env.KUBECOVE_MEMORY_FIXTURE, fixturePodRows: Number(process.env.KUBECOVE_MEMORY_FIXTURE_ROWS), sourceSha: process.env.KUBECOVE_PROFILE_SHA, sourceDirty: process.env.KUBECOVE_PROFILE_DIRTY === "true", stages, limitations: ["Isolated loopback Kubernetes fixture, no live cluster", "DOM events exercise UI handlers; physical pointer input is unverified", "JSON payload estimates are not heap allocation sizes", "WebView garbage collection is not forced", "Driver and observer overhead are included"] };
		const save = () => writeFile(join(artifacts, "frontend-memory.json"), `${JSON.stringify(report, null, 2)}\n`);
		await save();
		async function capture(stage: string, started = performance.now()) {
			const frontend = await browser.execute(() => {
				// SAFETY: Chromium exposes an optional heap extension; WebKit does not.
				const memory = (performance as Performance & { memory?: { usedJSHeapSize: number } }).memory;
				const heapBytes = memory?.usedJSHeapSize ?? null;
				if (!window.__KUBECOVE_MEMORY_PROFILE__) throw new Error("Memory instrumentation is unavailable");
				return {
					atMs: performance.now(), queries: window.__KUBECOVE_MEMORY_PROFILE__(), heapBytes,
					viewport: { width: innerWidth, height: innerHeight, scale: devicePixelRatio },
					renderedResourceActions: document.querySelectorAll('[aria-label^="Open resource "]').length,
					flowNodes: document.querySelectorAll(".svelte-flow__node").length,
					yamlUtf8Bytes: new TextEncoder().encode(document.querySelector(".cm-content")?.textContent ?? "").byteLength,
				};
			});
			const usage = await browser.execute(() => window.__TAURI__.core.invoke<AppUsageMetrics>("get_app_usage_metrics")).catch(() => null);
			stages.push({ stage, durationMs: performance.now() - started, ...frontend, memory: startupReport([], frontend.heapBytes, usage).memory });
			await save();
			if (artifacts && ["first-resource-rows", "yaml", "topology-open"].includes(stage)) await browser.saveScreenshot(join(artifacts, `${stage}.png`));
			return frontend;
		}
		async function clickLabel(label: string) {
			await browser.waitUntil(async () => browser.execute((label) => Boolean(document.querySelector(`[aria-label="${label}"]`)), label));
			await browser.execute((label) => document.querySelector<HTMLElement>(`[aria-label="${label}"]`)?.click(), label);
		}
		async function clickText(text: string) {
			await browser.waitUntil(async () => browser.execute((text) => Array.from(document.querySelectorAll<HTMLButtonElement>("button"))
				.some((button) => button.textContent?.trim() === text && !button.disabled), text), { timeout: 30_000, timeoutMsg: `Button did not become available: ${text}` });
			await browser.execute((text) => Array.from(document.querySelectorAll<HTMLButtonElement>("button"))
				.find((button) => button.textContent?.trim() === text && !button.disabled)?.click(), text);
		}
		await browser.waitUntil(async () => browser.execute(() => performance.getEntriesByName("kubecove:startup:launcher-ready").length > 0), { timeout: 30_000 });
		await capture("launcher-idle");
		let started = performance.now();
		await browser.execute(() => {
			const input = document.querySelector<HTMLInputElement>("#workspace-name");
			if (!input) throw new Error("Workspace form is missing");
			input.value = "Memory fixture";
			input.dispatchEvent(new Event("input", { bubbles: true }));
		});
		await clickText("Create workspace");
		await clickText("Resources");
		await browser.waitUntil(async () => browser.execute(() => Boolean(document.querySelector('[aria-label^="Open resource profile-pod-"]'))), { timeout: 30_000 });
		const rows = await capture("first-resource-rows", started);
		expect(rows.queries.roots.find((root) => root.root === "resources")?.items).toBeGreaterThanOrEqual(report.fixturePodRows);
		started = performance.now();
		await browser.waitUntil(async () => browser.execute((rows) => (window.__KUBECOVE_MEMORY_PROFILE__?.(false).roots.find((root) => root.root === "resource-metrics")?.items ?? 0) >= rows, report.fixturePodRows), { timeout: 30_000, timeoutMsg: "Fixture Pod metrics were not loaded" });
		await capture("metrics-ready", started);
		started = performance.now();
		await browser.execute(() => document.querySelector<HTMLElement>('[aria-label^="Open resource profile-pod-"]')?.click());
		await browser.waitUntil(async () => browser.execute(() => Boolean(document.querySelector('[aria-label="Close resource details"]'))));
		await capture("resource-detail", started);
		started = performance.now();
		await clickText("YAML");
		await browser.waitUntil(async () => browser.execute(() => Boolean(document.querySelector('[role="tab"][data-value="yaml"][data-state="active"]') && document.querySelector(".cm-content"))));
		await capture("yaml", started);
		await clickLabel("Close resource details");
		if (await browser.execute(() => Boolean(document.querySelector('[aria-label="Collapse ownership map"]')))) await clickLabel("Collapse ownership map");
		started = performance.now();
		if (await browser.execute(() => Boolean(document.querySelector('[aria-label="Show ownership map"]')))) await clickLabel("Show ownership map");
		await browser.waitUntil(async () => browser.execute(() => Boolean(document.querySelector(".svelte-flow"))), { timeout: 30_000 });
		await capture("topology-open", started);
		started = performance.now();
		await clickLabel("Collapse ownership map");
		await capture("topology-closed", started);
		started = performance.now();
		await clickLabel("Open workspaces");
		await browser.waitUntil(async () => browser.execute(() => Boolean(document.querySelector("#workspace-name"))));
		await capture("workspace-switch-to-launcher", started);
		// Reopen the same workspace to measure the existing warm back-navigation contract.
		started = performance.now();
		await clickText("Open");
		await clickText("Resources");
		await browser.waitUntil(async () => browser.execute(() => Boolean(document.querySelector('[aria-label^="Open resource profile-pod-"]'))));
		await capture("warm-back-navigation", started);
		await clickLabel("Open workspaces");
		await browser.waitUntil(async () => browser.execute(() => Boolean(document.querySelector("#workspace-name"))));
		const inactive = await capture("inactive-before-collection");
		for (const root of LARGE_QUERY_ROOTS) {
			const row = inactive.queries.roots.find((row) => row.root === root);
			expect(row?.queries).toBeGreaterThan(0);
			expect(row?.inactive).toBe(row?.queries);
			expect(row?.unobserved).toBe(row?.queries);
		}
		started = performance.now();
		await browser.waitUntil(async () => browser.execute((roots) => {
			const snapshot = window.__KUBECOVE_MEMORY_PROFILE__?.(false);
			return Boolean(snapshot?.roots.every((root) => !roots.some((name) => name === root.root) || root.queries === 0));
		}, [...LARGE_QUERY_ROOTS]), { timeout: LARGE_QUERY_GC_TIME_MS + (browser.options.waitforTimeout ?? 0), timeoutMsg: "Configured inactive queries were not collected" });
		await capture("inactive-after-collection", started);
		refetchAfterCollection = async () => {
			const started = performance.now();
			await clickText("Open");
			await clickText("Resources");
			await browser.waitUntil(async () => browser.execute(() => Boolean(document.querySelector('[aria-label^="Open resource profile-pod-"]'))), { timeout: 30_000 });
			await capture("post-collection-refetch", started);
			report.passed = true;
			await save();
		};
	});
	it("records refetch after collection in the same app session", async () => {
		if (!refetchAfterCollection) throw new Error("The collection scenario did not finish");
		await refetchAfterCollection();
	}).timeout(90_000);
});
