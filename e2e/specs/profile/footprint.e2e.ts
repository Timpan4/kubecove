import { writeFile } from "node:fs/promises";
import { join } from "node:path";
import { browser, expect } from "@wdio/globals";
import { describe, it } from "mocha";
import type { AppUsageMetrics } from "../../../src/lib/types";
import { startupReport } from "../../harness/startup-report";

describe("release footprint startup companion", () => {
	it("inspects fixture rows and YAML with the original permissions", async () => {
		const artifacts = process.env.KUBECOVE_E2E_ARTIFACTS;
		if (!artifacts) throw new Error("Artifact directory is required");
		const fonts = new Set<string>(JSON.parse(process.env.KUBECOVE_FOOTPRINT_FONTS ?? "[]"));
		const stages: object[] = [];
		async function capture(stage: string) {
			const frontend = await browser.execute((fonts) => {
				const loaded = Array.from(document.fonts).filter((font) => font.status === "loaded");
				const files = new Set<string>();
				const normalized = (value: string) => value.replace(/[\s"']/g, "").toLowerCase();
				function inspectRules(rules: CSSRuleList) {
					for (const rule of Array.from(rules)) {
						if (rule instanceof CSSFontFaceRule && loaded.some((font) => normalized(font.family) === normalized(rule.style.getPropertyValue("font-family"))
							&& normalized(font.unicodeRange) === normalized(rule.style.getPropertyValue("unicode-range")))) {
							const url = /url\(["']?([^"'()]+)/.exec(rule.style.getPropertyValue("src"))?.[1];
							const name = url?.split("/").at(-1)?.split("?")[0];
							if (name && fonts.includes(name)) files.add(name);
						} else if (rule instanceof CSSGroupingRule) inspectRules(rule.cssRules);
					}
				}
				for (const sheet of Array.from(document.styleSheets)) inspectRules(sheet.cssRules);
				return {
				entries: performance.getEntries().filter((entry) => entry.name.startsWith("kubecove:"))
					.map(({ name, startTime, duration }) => ({ name, startTime, duration })),
				fonts: [...files].sort(), fontObservation: "loaded CSS FontFace matched to its bundled src, not request timing",
				resourceTimingEntries: performance.getEntriesByType("resource").length,
				fontFaces: Array.from(document.fonts).map((font) => ({ family: font.family, status: font.status, unicodeRange: font.unicodeRange })),
				viewport: { width: innerWidth, height: innerHeight, scale: devicePixelRatio },
			};
			}, [...fonts]);
			expect(frontend.fonts.length).toBeGreaterThan(0);
			const usage = await browser.execute(() => window.__TAURI__.core.invoke<AppUsageMetrics>("get_app_usage_metrics"));
			stages.push({ stage, ...frontend, ...startupReport(frontend.entries, null, usage) });
		}
		async function clickText(text: string) {
			await browser.waitUntil(async () => browser.execute((text) => Array.from(document.querySelectorAll<HTMLButtonElement>("button"))
				.some((button) => button.textContent?.trim() === text && !button.disabled), text));
			await browser.execute((text) => Array.from(document.querySelectorAll<HTMLButtonElement>("button"))
				.find((button) => button.textContent?.trim() === text && !button.disabled)?.click(), text);
		}
		await browser.waitUntil(async () => browser.execute(() => performance.getEntriesByName("kubecove:startup:launcher-ready").length > 0));
		await browser.execute(async () => { await document.fonts.ready; return true; });
		await capture("launcher");
		await browser.execute(() => {
			const input = document.querySelector<HTMLInputElement>("#workspace-name");
			if (!input) throw new Error("Workspace form is unavailable");
			input.value = "Footprint fixture";
			input.dispatchEvent(new Event("input", { bubbles: true }));
		});
		await clickText("Create workspace");
		await clickText("Resources");
		await browser.waitUntil(async () => browser.execute(() => Boolean(document.querySelector('[aria-label^="Open resource profile-pod-"]'))));
		await browser.execute(async () => { await document.fonts.ready; return true; });
		await capture("workspace");
		await browser.execute(() => document.querySelector<HTMLElement>('[aria-label^="Open resource profile-pod-"]')?.click());
		await clickText("YAML");
		await browser.waitUntil(async () => browser.execute(() => Boolean(document.querySelector('[role="tab"][data-value="yaml"][data-state="active"]') && document.querySelector(".cm-content"))));
		await capture("yaml");
		await browser.saveScreenshot(join(artifacts, "yaml.png"));
		// An invalid resource ID exercises updater dispatch without downloading or installing.
		const plugins = await browser.execute(async () => {
			const results: Array<{ command: string; error: string }> = [];
			for (const command of ["plugin:updater|download", "plugin:updater|install", "plugin:updater|download_and_install"]) {
				try { await window.__TAURI__.core.invoke(command, { rid: -1 }); }
				catch (error) { results.push({ command, error: String(error) }); }
			}
			return results;
		});
		expect(plugins.length).toBe(3);
		for (const plugin of plugins) {
			expect(plugin.error).not.toMatch(/not found|not allowed|forbidden/i);
		}
		await writeFile(join(artifacts, "footprint-startup.json"), `${JSON.stringify({ passed: true, scenario: "fresh-65-pod-fixture-launcher-resources-yaml", build: "release-with-e2e-feature", stages, plugins, limitations: ["DOM events and E2E driver overhead", "WebKit JS heap is unavailable", "Production sizes are measured separately", "Restart and update execution are not performed; preserved command registration is checked separately"] }, null, 2)}\n`);
	});
});
