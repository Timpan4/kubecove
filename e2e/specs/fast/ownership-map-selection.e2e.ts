import { writeFile } from "node:fs/promises";
import { join } from "node:path";
import { $, browser, expect } from "@wdio/globals";
import { describe, it } from "mocha";

const FIXTURE = "/e2e/fixtures/ownership-map-selection.ts";

describe("ownership map selection rendering", () => {
	it("keeps topology errors accessible while showing the last graph and after recovery", async () => {
		const artifacts = process.env.KUBECOVE_E2E_ARTIFACTS;
		if (!artifacts) throw new Error("Artifact directory is required");
		await browser.url("/");
		await browser.execute(async (fixture) => (await import(/* @vite-ignore */ fixture)).mountOwnershipMap(true), FIXTURE);
		const disclosure = $("summary=Map refresh failed");
		const empty = $("div=No ownership graph");
		const focusControl = (label: string) => browser.execute((text) => {
			const control = [...document.querySelectorAll<HTMLElement>("button, summary")]
				.find((element) => element.textContent?.trim() === text);
			if (!control) throw new Error(`Missing map control: ${text}`);
			control.focus();
		}, label);
		const focusText = () => browser.execute(() => document.activeElement?.textContent?.trim());
		const graph = () => browser.execute(async (fixture) =>
			(await import(/* @vite-ignore */ fixture)).settleOwnershipMap(), FIXTURE);
		const refresh = (empty: boolean) => browser.execute(async (fixture, empty) =>
			(await import(/* @vite-ignore */ fixture)).refreshOwnershipMapTopology(empty), FIXTURE, empty);
		try {
			await browser.execute(() => {
				const harness = document.querySelector<HTMLElement>("[data-ownership-map-harness]");
				if (!harness) throw new Error("Ownership map fixture is missing");
				harness.style.width = "280px";
				harness.style.height = "432px";
			});
			for (const [label, retainedEmpty, recoveredEmpty] of [
				["graph", false, false],
				["graph-to-empty", false, true],
				["empty", true, true],
			] as const) {
				if (retainedEmpty) await refresh(true);
				const edgeCount = retainedEmpty ? 0 : (await graph()).edges;
				if (!retainedEmpty) {
					expect(edgeCount).toBeGreaterThan(0);
					await expect($("div=Last observed topology warning")).toBeDisplayed();
				}
				await browser.execute(async (fixture) => (await import(/* @vite-ignore */ fixture)).failOwnershipMapTopology(), FIXTURE);
				await expect(disclosure).toBeDisplayed();
				if (retainedEmpty) {
					await focusControl("Network Flow");
					await browser.keys("Tab");
				}
				else {
					await focusControl("skip graph");
					await browser.keys("Enter");
					await browser.keys("Tab");
				}
				expect(await focusText()).toBe("Map refresh failed");
				await browser.keys("Enter");
				await browser.keys("Tab");
				expect(await focusText()).toBe("Technical detail");
				await browser.keys("Enter");
				const technicalDetail = $("details[open] pre");
				await expect(technicalDetail).toBeDisplayed();
				expect(await technicalDetail.getText()).toContain("Topology request unavailable");
				expect(await browser.execute(() => {
					const detail = document.querySelector("details[open] pre");
					const disclosure = detail?.closest("details[data-map-refresh-error]");
					return Boolean(detail && disclosure && detail.clientHeight === detail.scrollHeight
						&& disclosure.clientWidth === disclosure.scrollWidth);
				})).toBe(true);
				await browser.keys("Tab");
				expect(await browser.execute(() => {
					const control = document.activeElement;
					if (control?.textContent?.trim() !== "Copy detail") return false;
					const bounds = control.getBoundingClientRect();
					return control.contains(document.elementFromPoint(
						bounds.left + bounds.width / 2, bounds.top + bounds.height / 2,
					));
				})).toBe(true);
				if (retainedEmpty) await expect(empty).toBeDisplayed();
				else {
					const retained = await graph();
					expect(retained.edges).toBe(edgeCount);
					expect(retained.edgeRemovals).toBe(0);
				}
				await browser.saveScreenshot(join(artifacts, `ownership-map-error-${label}.png`));
				await refresh(recoveredEmpty);
				await expect(disclosure).not.toExist();
				if (recoveredEmpty) await expect(empty).toBeDisplayed();
				else expect((await graph()).edges).toBe(edgeCount);
				expect(await focusText()).toBe("Ownership Map");
			}
		} finally {
			await browser.execute(async (fixture) => (await import(/* @vite-ignore */ fixture)).stopOwnershipMap(), FIXTURE);
		}
	});

	it("keeps edges mounted and skips re-measuring nodes across selection and refresh", async () => {
		const artifacts = process.env.KUBECOVE_E2E_ARTIFACTS;
		if (!artifacts) throw new Error("Artifact directory is required");
		await browser.url("/");
		await browser.execute(async (fixture) => (await import(/* @vite-ignore */ fixture)).mountOwnershipMap(), FIXTURE);
		const settle = () => browser.execute(async (fixture) => (await import(/* @vite-ignore */ fixture)).settleOwnershipMap(), FIXTURE);
		const act = (action: "select" | "refresh", nodeId: string | null = null) =>
			browser.execute(async (fixture, action, nodeId) => {
				const map = await import(/* @vite-ignore */ fixture);
				if (action === "select") map.selectOwnershipMapNode(nodeId);
				else map.refreshOwnershipMapTopology();
			}, FIXTURE, action, nodeId);
		try {
			const mounted = await settle();
			expect(mounted.edges).toBe(45);
			const steps: Array<[string, Awaited<ReturnType<typeof settle>>]> = [];
			for (const [label, action, nodeId] of [
				["select first Deployment", "select", "Deployment/api-0"],
				["select another Deployment", "select", "Deployment/api-7"],
				["select a Pod", "select", "Pod/api-7-rs-b"],
				["clear selection", "select", null],
				["refresh topology", "refresh", null],
			] as const) {
				await act(action, nodeId);
				steps.push([label, await settle()]);
			}
			await writeFile(join(artifacts, "ownership-map-selection.json"), JSON.stringify({ mounted, steps: Object.fromEntries(steps) }, null, 2));
			for (const [, counts] of steps) {
				expect(counts.edges).toBe(45);
				expect(counts.edgeRemovals).toBe(0);
				expect(counts.handleMeasurements).toBe(0);
			}
		} finally {
			await browser.execute(async (fixture) => (await import(/* @vite-ignore */ fixture)).stopOwnershipMap(), FIXTURE);
		}
	});
});
