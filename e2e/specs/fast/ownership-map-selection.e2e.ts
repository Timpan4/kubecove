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
		await browser.execute(async (fixture) => (await import(/* @vite-ignore */ fixture)).mountOwnershipMap(), FIXTURE);
		await browser.execute(() => {
			const harness = document.querySelector<HTMLElement>("[data-ownership-map-harness]");
			if (harness) {
				harness.style.width = "280px";
				harness.style.height = "432px";
			}
		});
		const disclosure = $("summary=Map refresh failed");
		const empty = $("div=No ownership graph");
		try {
			for (const retainedEmpty of [false, true]) {
				await browser.execute(async (fixture) => (await import(/* @vite-ignore */ fixture)).failOwnershipMapTopology(), FIXTURE);
				await disclosure.click();
				const details = $("summary=Technical detail");
				await details.click();
				await expect($("pre=Topology request unavailable")).toBeDisplayed();
				if (retainedEmpty) await expect(empty).toBeDisplayed();
				else expect(await browser.execute(() => document.querySelectorAll(".svelte-flow__edge").length)).toBe(45);
				await browser.saveScreenshot(join(artifacts, `ownership-map-error-${retainedEmpty ? "empty" : "graph"}.png`));
				await browser.execute(async (fixture) => (await import(/* @vite-ignore */ fixture)).refreshOwnershipMapTopology(true), FIXTURE);
				await expect(disclosure).not.toExist();
				await expect(empty).toBeDisplayed();
				expect(await browser.execute(() => document.activeElement?.textContent?.trim())).toBe("Ownership Map");
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
