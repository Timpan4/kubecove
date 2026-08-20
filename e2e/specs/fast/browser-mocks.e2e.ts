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
		const originalSize = await browser.getWindowSize();
		await browser.sendCommandAndGetResult("Emulation.setEmulatedMedia", {
			features: [{ name: "prefers-reduced-motion", value: "reduce" }],
		});

		try {
			expect(
				await browser.execute(() =>
					window.matchMedia("(prefers-reduced-motion: reduce)").matches,
				),
			).toBe(true);
			await $("button=Resources").click();
			const graph = await $(".svelte-flow");
			await graph.waitForDisplayed();

			for (const [width, height] of [
				[1100, 600],
				[1440, 600],
			] as const) {
				await browser.setWindowSize(width, height);
				const pageScroller = await $(".overflow-auto.h-full:not(.scrollbar-classic)");
				await browser.execute((element: HTMLElement) => {
					element.scrollTop = 0;
				}, pageScroller);
				await browser
					.action("wheel")
					.scroll({ origin: graph, deltaX: 0, deltaY: 300, duration: 0 })
					.perform();
				await browser.waitUntil(
					async () =>
						(await browser.execute(
							(element: HTMLElement) => element.scrollTop,
							pageScroller,
						)) > 0,
				);
			}

			await $("button=skip graph").click();
			expect(
				await browser.execute(() => document.activeElement?.getAttribute("tabindex")),
			).toBe("-1");
		} finally {
			await browser.setWindowSize(originalSize.width, originalSize.height);
			await browser.sendCommandAndGetResult("Emulation.setEmulatedMedia", { features: [] });
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
