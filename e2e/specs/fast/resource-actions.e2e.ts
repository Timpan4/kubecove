import { join } from "node:path";
import { $, browser, expect } from "@wdio/globals";
import { beforeEach, describe, it } from "mocha";

/*
 * Test brief. Cover the failure modes that can make a guarded resource action
 * unsafe or silently lose work: pending edits must survive an independent
 * action, preview errors must not allow execution, deletes must still require
 * the exact typed name when review is skipped, and switching target/source
 * must invalidate pending review state. The API server remains authoritative.
 */
describe("resource Actions review", () => {
	beforeEach(async () => {
		await browser.url("/");
		await browser.execute(() => localStorage.clear());
		await browser.refresh();
		await $('button[aria-label="Open settings"]').click();
		await $('input[aria-label="Search settings"]').setValue("Skip change review");
		const skipReview = $('button[aria-label="Skip change review"]');
		if ((await skipReview.getAttribute("aria-checked")) === "true") await skipReview.click();
		await $("button=Back to app").click();
		await $("#workspace-name").setValue("Fast Mock Lab");
		const create = await $("button=Create workspace");
		await create.waitForEnabled();
		await create.click();
		await $$("button=Resources")[0].click();
		await $('input[aria-label="Search resources"]').setValue("payments-api");
		await $('//tr[td[normalize-space()="Deployment"]]//button[@aria-label="Open resource payments-api"]').click();
		await $('[role="tab"]=Actions').click();
	});

	async function enableSkipReview() {
		await $('button[aria-label="Open settings"]').click();
		await $("button=Advanced").click();
		await $('[aria-label="Skip change review"]').click();
		const warning = await $('[role="alert"]');
		await warning.$("button=Skip review").click();
		await $("button=Back to app").click();
		await $$("button=Resources")[0].click();
		await $('input[aria-label="Search resources"]').setValue("payments-api");
		await $('//tr[td[normalize-space()="Deployment"]]//button[@aria-label="Open resource payments-api"]').click();
		await $('[role="tab"]=Actions').click();
	}

	it("keeps a pending container edit when a separate restart succeeds", async () => {
		const imageRow = await $('[data-resource-action="image"]');
		await imageRow.$("button=Edit").click();
		const image = await imageRow.$('input[aria-label="Container image"]');
		await image.setValue("ghcr.io/example/payments-api:reviewed");
		await expect(imageRow).toHaveText(expect.stringContaining("Pending"));
		const replicasRow = await $('[data-resource-action="replicas"]');
		await replicasRow.$("button=Edit").click();
		await replicasRow.$('input[aria-label="Desired replicas"]').setValue("3");
		await expect(replicasRow).toHaveText(expect.stringContaining("Pending"));

		const environmentRow = await $('[data-resource-action="environment"]');
		await environmentRow.$("button=Edit").click();
		const newVariableName = await environmentRow.$('input[aria-label^="New environment name "]');
		const newVariableValue = await environmentRow.$('input[aria-label^="New environment value "]');
		await newVariableName.setValue("ACTIONS_REVIEW");
		await newVariableValue.setValue("enabled");
		await environmentRow.$("button=Add").click();
		const resourcesRow = await $('[data-resource-action="resources"]');
		await resourcesRow.$("button=Edit").click();
		await resourcesRow.$('input[aria-label="Request value api cpu"]').setValue("100m");
		await resourcesRow.$('input[aria-label="New request name api"]').setValue("ephemeral-storage");
		await resourcesRow.$('input[aria-label="New request value api"]').setValue("1Gi");
		await resourcesRow.$$("button=Add")[0].click();
		await expect(resourcesRow).toHaveText(expect.stringContaining("Pending"));

		const labelsRow = await $('[data-resource-action="labels"]');
		await labelsRow.$("button=Edit").click();
		await labelsRow.$('input[aria-label="New labels key"]').setValue("actions-review");
		await labelsRow.$('input[aria-label="New labels value"]').setValue("enabled");
		await labelsRow.$("button=Add").click();
		const annotationsRow = await $('[data-resource-action="annotations"]');
		await annotationsRow.$("button=Edit").click();
		await annotationsRow.$('input[aria-label="New annotations key"]').setValue("actions-review");
		await annotationsRow.$('input[aria-label="New annotations value"]').setValue("enabled");
		await annotationsRow.$("button=Add").click();

		await $('[data-resource-action="restart"]').$("button=Restart").click();
		const restartReview = await $('[role="dialog"]');
		await expect(restartReview).toHaveText(expect.stringContaining("restart"));
		await restartReview.$("button=Apply").click();

		await expect(imageRow).toHaveText(expect.stringContaining("Pending"));
		await $("button=Check changes").click();
		const review = await $('[role="dialog"]');
		const diff = await review.$('pre[aria-label="Server result diff"]');
		await expect(diff).toHaveText(expect.stringContaining("ghcr.io/example/payments-api:reviewed"));
		await expect(diff).toHaveText(expect.stringContaining("replicas: 3"));
		await expect(diff).toHaveText(expect.stringContaining("cpu: 100m"));
		await expect(diff).toHaveText(expect.stringContaining("ephemeral-storage: 1Gi"));
		await expect(diff).toHaveText(expect.stringContaining("ACTIONS_REVIEW"));
		await expect(diff).toHaveText(expect.stringContaining("actions-review"));
	});

	it("filters actions and keeps delete confirmation when review skipping is enabled", async () => {
		const filter = await $('input[aria-label="Resource action filter"]');
		await expect($('[data-resource-action="annotations"]')).toBeDisplayed();
		await filter.setValue("annotations");
		await expect($('[data-resource-action="annotations"]')).toBeDisplayed();
		await expect($('[data-resource-action="restart"]')).not.toBeDisplayed();
		await filter.clearValue();

		await $('button[aria-label="Open settings"]').click();
		await $("button=Advanced").click();
		await $('[aria-label="Skip change review"]').click();
		const warning = await $('[role="alert"]');
		await expect(warning).toBeDisplayed();
		await warning.$("button=Skip review").click();
		await $("button=Back to app").click();
		await $$("button=Resources")[0].click();
		await $('input[aria-label="Search resources"]').setValue("payments-api");
		await $('//tr[td[normalize-space()="Deployment"]]//button[@aria-label="Open resource payments-api"]').click();
		await $('[role="tab"]=Actions').click();

		await $('[data-resource-action="delete"]').$("button=Delete").click();
		const review = await $('[role="dialog"]');
		await expect(review.$('input[aria-label="Type resource name to confirm"]')).toBeDisplayed();
		await expect(review.$("button=Delete")).toBeDisabled();
		await review.$('input[aria-label="Type resource name to confirm"]').setValue("payments-api");
		await expect(review.$("button=Delete")).toBeEnabled();
	});

	it("keeps a failed dry run in review without an execution action", async () => {
		const imageRow = await $('[data-resource-action="image"]');
		await imageRow.$("button=Edit").click();
		await imageRow.$('input[aria-label="Container image"]').setValue(" ");
		await $("button=Check changes").click();

		const review = await $('[role="dialog"]');
		await expect(review).toHaveText(expect.stringContaining("Could not complete the check or confirm the action."));
		await expect(review.$("button=Apply")).not.toBeDisplayed();
		await expect(review.$("button=Cancel")).toBeDisplayed();
	});

	it("previews and executes rollback from an owned revision with its complete template", async () => {
		const artifacts = process.env.KUBECOVE_E2E_ARTIFACTS;
		if (!artifacts) throw new Error("Resource Actions E2E artifact directory is required");
		await browser.saveScreenshot(join(artifacts, "resource-actions-rows.png"));
		const rollbackRow = await $('[data-resource-action="rollback"]');
		await rollbackRow.$("button=Roll back").click();
		await $('input[name="resource-action-revision"][value="2"]').click();
		await rollbackRow.$("button=Review rollback").click();

		const review = await $('[role="dialog"]');
		const serverResult = await review.$('pre[aria-label="Server result diff"]');
		await expect(serverResult).toBeDisplayed();
		await browser.saveScreenshot(join(artifacts, "rollback-review-dialog.png"));
		const restoredTemplate = await browser.execute(() =>
			Array.from(document.querySelectorAll('[aria-label="Server result diff"] span.text-emerald-300'))
				.map((line) => line.textContent ?? "")
				.join("\n"),
		);
		expect(restoredTemplate).toContain("image: registry.example/api:v0");
		expect(restoredTemplate).toContain("mock.example/template: revision-2");
		expect(restoredTemplate).toContain("value: revision-2");
		expect(restoredTemplate).toContain("cpu: 50m");
		expect(restoredTemplate).toContain("memory: 128Mi");
		expect(restoredTemplate).toContain("name: payments-api-revision-2");
		await review.$("button=Apply").click();
		await expect($("body")).toHaveText(expect.stringContaining("Simulated rollback"));
	});

	it("applies a pending edit inline without opening review when skipping is enabled", async () => {
		await enableSkipReview();
		const imageRow = await $('[data-resource-action="image"]');
		await imageRow.$("button=Edit").click();
		await imageRow.$('input[aria-label="Container image"]').setValue("ghcr.io/example/payments-api:skipped");
		const outcome = await browser.execute(() => new Promise<{ dialogSeen: boolean; missingCheckButton?: boolean }>((done) => {
			let dialogSeen = false;
			const observer = new MutationObserver(() => {
				if (document.querySelector('[role="dialog"]')) dialogSeen = true;
				const applied = Array.from(document.querySelectorAll('[role="status"]')).some((element) =>
					element.textContent?.includes("Simulated edit"),
				);
				if (!applied) return;
				observer.disconnect();
				done({ dialogSeen });
			});
			observer.observe(document.body, { childList: true, subtree: true, characterData: true });
			const checkChanges = Array.from(document.querySelectorAll<HTMLButtonElement>("button")).find(
				(button) => button.textContent?.trim() === "Check changes",
			);
			if (!checkChanges) {
				observer.disconnect();
				done({ dialogSeen, missingCheckButton: true });
				return;
			}
			checkChanges.click();
		}));
		expect(outcome).toEqual({ dialogSeen: false });
		await expect($('[role="dialog"]')).not.toBeDisplayed();
		await expect(imageRow).not.toHaveText(expect.stringContaining("Pending"));
	});

	it("opens review with the error when a skipped dry run fails", async () => {
		await enableSkipReview();
		const imageRow = await $('[data-resource-action="image"]');
		await imageRow.$("button=Edit").click();
		await imageRow.$('input[aria-label="Container image"]').setValue(" ");
		await $("button=Check changes").click();

		const review = await $('[role="dialog"]');
		await expect(review).toHaveText(expect.stringContaining("Could not complete the check or confirm the action."));
		await expect(review.$("button=Apply")).not.toBeDisplayed();
	});

	it("traps keyboard focus inside an open server review", async () => {
		const imageRow = await $('[data-resource-action="image"]');
		await imageRow.$("button=Edit").click();
		await imageRow.$('input[aria-label="Container image"]').setValue("ghcr.io/example/payments-api:keyboard-review");
		await $("button=Check changes").click();
		const review = await $('[role="dialog"]');
		await review.waitForDisplayed();
		await browser.keys(["TAB", "TAB", "TAB", "TAB", "TAB"]);
		const focusStayedInReview = await browser.execute(() =>
			document.activeElement?.closest('[role="dialog"]') !== null,
		);
		await expect(focusStayedInReview).toBe(true);
	});
});
