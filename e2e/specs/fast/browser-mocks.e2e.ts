import { $, browser, expect } from "@wdio/globals";
import { before, describe, it } from "mocha";

describe("browser mock inspection", () => {
	before(async () => {
		await browser.url("/");
		await browser.execute(() => localStorage.clear());
		await browser.refresh();
	});

	it("shows deterministic mock context and resources", async () => {
		await expect(browser).toHaveTitle(expect.stringContaining("KubeCove"));
		await expect($("body")).toHaveText(expect.stringContaining("mock-dev"));
		await $("#workspace-name").setValue("Fast Mock Lab");
		const create = await $("button=Create workspace");
		await create.waitForEnabled();
		await create.click();
		await $("button=Resources").click();
		await expect($("body")).toHaveText(expect.stringContaining("payments-api"));
	});

	it("opens adjacent Argo Application details by accessible name", async () => {
		await $('button[aria-label="Open workspace navigation"]').click();
		const navigation = await $('[data-slot="sheet-content"]');
		await navigation.$('[role="treeitem"]*=GitOps').click();

		const firstDetails = await $('button[aria-label="Open details for platform-argocd"]');
		await firstDetails.click();
		await expect($('button[aria-label="Pin Application platform-argocd"]')).toBeDisplayed();
		await $('button[aria-label="Close resource details"]').click();

		const secondDetails = await $('button[aria-label="Open details for platform-cilium"]');
		await browser.execute((element: HTMLElement) => element.focus(), secondDetails);
		await browser.keys("Enter");
		await expect($('button[aria-label="Pin Application platform-cilium"]')).toBeDisplayed();
	});

	it("identifies the typed browser-only source", async () => {
		await $('button[aria-label="Open settings"]').click();
		await $("button=Kubeconfig").click();
		await expect($("body")).toHaveText(expect.stringContaining("Browser mock"));
	});
});
