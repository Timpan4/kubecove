import { $, browser, expect } from "@wdio/globals";
import { beforeEach, describe, it } from "mocha";

describe("browser mock inspection", () => {
	beforeEach(async () => {
		await browser.url("/");
		await browser.execute(() => localStorage.clear());
		await browser.refresh();
		await $("#workspace-name").setValue("Fast Mock Lab");
		const create = await $("button=Create workspace");
		await create.waitForEnabled();
		await create.click();
	});

	it("shows deterministic mock context and resources", async () => {
		await expect(browser).toHaveTitle(expect.stringContaining("KubeCove"));
		await expect($("body")).toHaveText(expect.stringContaining("mock-dev"));
		await $("button=Resources").click();
		await expect($("body")).toHaveText(expect.stringContaining("payments-api"));
	});

	it("lets page scrolling bypass the resource graph", async () => {
		await $("button=Resources").click();
		const graph = await $(".svelte-flow");
		await graph.waitForDisplayed();
		const originalSize = await browser.getWindowSize();

		try {
			for (const [width, height] of [
				[1100, 800],
				[1440, 900],
			] as const) {
				await browser.setWindowSize(width, height);
				const wheelResult = await browser.execute((element: HTMLElement) => {
					const viewport = element.querySelector<HTMLElement>(".svelte-flow__viewport");
					const transform = viewport?.style.transform;
					const wheel = new WheelEvent("wheel", {
						bubbles: true,
						cancelable: true,
						deltaY: 300,
					});
					return {
						dispatched: element.dispatchEvent(wheel),
						defaultPrevented: wheel.defaultPrevented,
						transformUnchanged: viewport?.style.transform === transform,
					};
				}, graph);
				expect(wheelResult).toEqual({
					dispatched: true,
					defaultPrevented: false,
					transformUnchanged: true,
				});
			}

			await $("button=skip graph").click();
			expect(
				await browser.execute(() => document.activeElement?.getAttribute("tabindex")),
			).toBe("-1");
		} finally {
			await browser.setWindowSize(originalSize.width, originalSize.height);
		}
	});

	it("opens adjacent Argo Application details by accessible name", async () => {
		await $('button[aria-label="Open workspace navigation"]').click();
		const navigation = await $('[data-slot="sheet-content"]');
		await navigation.$('[role="treeitem"]*=GitOps').click();

		const firstDetails = await $('button[aria-label="Open details for Application platform-argocd in argocd"]');
		await firstDetails.click();
		await expect($('button[aria-label="Pin Application platform-argocd"]')).toBeDisplayed();
		await $('button[aria-label="Close resource details"]').click();

		const keyboardDetails = await $('button[aria-label="Open details for Application platform-argocd in argocd"]');
		await keyboardDetails.scrollIntoView();
		await browser.execute((element: HTMLElement) => element.focus(), keyboardDetails);
		await browser.keys("Enter");
		await expect($('button[aria-label="Pin Application platform-argocd"]')).toBeDisplayed();
		await $('button[aria-label="Close resource details"]').click();

		const secondDetails = await $('button[aria-label="Open details for Application platform-cilium in argocd"]');
		await secondDetails.click();
		await expect($('button[aria-label="Pin Application platform-cilium"]')).toBeDisplayed();
		await $('button[aria-label="Close resource details"]').click();
	});

	it("identifies the typed browser-only source", async () => {
		await $('button[aria-label="Open settings"]').click();
		await $("button=Kubeconfig").click();
		await expect($("body")).toHaveText(expect.stringContaining("Browser mock"));
	});
});
