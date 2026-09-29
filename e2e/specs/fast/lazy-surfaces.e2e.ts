import { $, browser, expect } from "@wdio/globals";
import { describe, it } from "mocha";
import { join } from "node:path";

describe("deferred application screens", () => {
	it("loads the workspace and details only when opened and restores YAML", async () => {
		await browser.url("/");
		await browser.execute(() => localStorage.clear());
		await browser.refresh();
		await $("#workspace-name").waitForDisplayed();
		const loaded = () => browser.execute(() => performance.getEntriesByType("resource").map((entry) => entry.name));
		expect((await loaded()).some((name) => name.includes("/WorkspaceShell.svelte"))).toBe(false);
		await $("#workspace-name").setValue("Deferred screens");
		const create = await $("button=Create workspace");
		await create.waitForEnabled();
		await create.click();
		await $("button=Resources").waitForDisplayed();
		expect((await loaded()).some((name) => name.includes("/WorkspaceShell.svelte"))).toBe(true);
		expect((await loaded()).some((name) => name.includes("/ResourceDetailPanel.svelte"))).toBe(false);
		for (const name of ["GitOpsSurface", "HelmSurface", "RbacSurface", "IncidentSurface", "LiveSessionsSurface", "SettingsSurface"]) {
			expect((await loaded()).some((url) => url.includes(`/${name}.svelte`))).toBe(false);
		}
		await $("button=Resources").click();
		await $('input[aria-label="Search resources"]').setValue("payments-api");
		await browser.execute(() => performance.clearResourceTimings());
		await $('button[aria-label="Open resource payments-api"]').click();
		await $('[role="tab"]=YAML').waitForDisplayed();
		await $('[role="tab"]=YAML').click();
		await expect($("body")).toHaveText(expect.stringContaining("apiVersion:"));
		expect((await loaded()).some((name) => name.includes("/ResourceDetailPanel.svelte"))).toBe(true);
		for (const name of ["GitOpsSurface", "HelmSurface", "RbacSurface", "IncidentSurface", "LiveSessionsSurface", "SettingsSurface"]) {
			expect((await loaded()).some((url) => url.includes(`/${name}.svelte`))).toBe(false);
		}
		await browser.refresh();
		await expect($('[role="tab"]=YAML')).toHaveAttribute("data-state", "active");
		await expect($("body")).toHaveText(expect.stringContaining("apiVersion:"));
	});

	it("recovers a failed workspace chunk without losing the selected workspace", async () => {
		await browser.url("/");
		await browser.execute(() => localStorage.clear());
		await browser.refresh();
		await $("#workspace-name").waitForDisplayed();
		await browser.sendCommandAndGetResult("Network.enable", {});
		await browser.sendCommandAndGetResult("Network.setBlockedURLs", { urls: ["*WorkspaceShell.svelte*"] });
		try {
			await $("#workspace-name").setValue("Retry workspace");
			const create = await $("button=Create workspace");
			await create.waitForEnabled();
			await create.click();
			await expect($("body")).toHaveText(expect.stringContaining("Could not load workspace."));
			await browser.sendCommandAndGetResult("Network.setBlockedURLs", { urls: [] });
			await $("button=Reload to retry workspace").click();
			await $("button=Resources").waitForDisplayed();
			await expect($("body")).toHaveText(expect.stringContaining("Retry workspace"));
		} finally {
			await browser.sendCommandAndGetResult("Network.setBlockedURLs", { urls: [] });
			await browser.sendCommandAndGetResult("Network.disable", {});
		}
	});

	it("opens each deferred area through workspace navigation", async () => {
		await browser.url("/");
		await browser.execute(() => localStorage.clear());
		await browser.refresh();
		await $("#workspace-name").setValue("Deferred areas");
		const create = await $("button=Create workspace");
		await create.waitForEnabled();
		await create.click();
		for (const [label, component, loadingLabel] of [
			["GitOps", "GitOpsSurface", "GitOps"],
			["Helm", "HelmSurface", "Helm"],
			["RBAC", "RbacSurface", "RBAC"],
			["Incidents", "IncidentSurface", "incidents"],
			["Port Forwards", "LiveSessionsSurface", "live sessions"],
		]) {
			await browser.execute(() => performance.clearResourceTimings());
			await $('button[aria-label="Open workspace navigation"]').click();
			const navigation = await $('[data-slot="sheet-content"]');
			await navigation.$(`[role="treeitem"]*=${label}`).click();
			await browser.waitUntil(() => browser.execute((name: string) =>
				performance.getEntriesByType("resource").some((entry) => entry.name.includes(`/${name}.svelte`)), component));
			await expect($("body")).not.toHaveText(expect.stringContaining(`Could not load`));
			await expect($("body")).not.toHaveText(expect.stringContaining(`Loading ${loadingLabel}…`));
			const artifacts = process.env.KUBECOVE_E2E_ARTIFACTS;
			if (artifacts) await browser.saveScreenshot(join(artifacts, `${component}.png`));
		}
		await browser.execute(() => performance.clearResourceTimings());
		await $('button[aria-label="Open settings"]').click();
		await browser.waitUntil(() => browser.execute(() =>
			performance.getEntriesByType("resource").some((entry) => entry.name.includes("/SettingsSurface.svelte"))));
		await expect($("body")).toHaveText(expect.stringContaining("Show exact timestamps"));
	});
});
