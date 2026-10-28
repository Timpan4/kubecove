import { $, browser, expect } from "@wdio/globals";
import { beforeEach, describe, it } from "mocha";

async function openArgoConnection() {
	await $('button[aria-label="Open workspace navigation"]').click();
	await $('[data-slot="sheet-content"]').$('[role="treeitem"]*=GitOps').click();
	const open = $("button=Argo CD connection");
	await open.waitForClickable();
	await open.click();
	await $('[role="radiogroup"][aria-label="Discovered Argo CD servers"]').waitForDisplayed();
}

describe("workspace Argo CD connection", () => {
	beforeEach(async () => {
		await browser.url("/");
		await browser.execute(() => localStorage.clear());
		await browser.refresh();
		await $("#workspace-name").setValue("Argo Mock Lab");
		const create = await $("button=Create workspace");
		await create.waitForEnabled();
		await create.click();
	});

	it("connects a discovered server after confirming its Pod and certificate, then reconnects on reload", async () => {
		await openArgoConnection();
		const preferred = $('button[role="radio"]*=argocd/argocd-server:443');
		await expect(preferred).toHaveAttribute("aria-checked", "true");
		await expect(preferred).toHaveText(expect.stringContaining("HTTPS"));

		const connect = $("button=Connect");
		await expect(connect).toBeDisabled();
		await $("#argo-token").setValue("mock-token");
		await $('button[aria-label="Remember credential in native keyring"]').click();
		await connect.click();

		const confirmation = $('[role="alert"]*=Confirm tunnel target');
		await confirmation.waitForDisplayed();
		await expect(confirmation).toHaveText(expect.stringContaining("argocd-server-7d9f8b6c5-x2k4q"));
		await expect(confirmation).toHaveText(expect.stringContaining(`SHA-256 ${Array(32).fill("3F").join(":")}`));
		await $("button=Confirm and connect").click();

		const saved = $('section[aria-label="Saved Argo CD connections"]');
		await expect(saved).toHaveText(expect.stringContaining("Connected as mock-user"));
		await expect(saved).toHaveText(expect.stringContaining("reconnects automatically"));
		const profiles = await browser.execute(
			() => JSON.parse(localStorage.getItem("kubecove-settings") ?? "{}").state.argoProfiles,
		);
		expect(profiles).toHaveLength(1);
		expect(profiles[0]).toMatchObject({
			clusterContext: "mock-dev",
			kubeconfigSourceKey: "browser-mock",
			rememberCredential: true,
			endpoint: { kind: "serviceTunnel", namespace: "argocd", serviceName: "argocd-server", servicePort: 443 },
		});
		expect(profiles[0].workspaceId).toEqual(expect.any(String));

		// The mock backend forgets live connections on reload, like an app restart.
		await browser.refresh();
		await openArgoConnection();
		await expect($('section[aria-label="Saved Argo CD connections"]')).toHaveText(
			expect.stringContaining("Connected as mock-user"),
		);
	});

	it("keeps Argo CD out of global Settings", async () => {
		await $('button[aria-label="Open settings"]').click();
		const sections = $('nav[aria-label="Settings sections"]');
		await sections.waitForDisplayed();
		await expect(sections).not.toHaveText(expect.stringContaining("Argo CD"));
	});
});
