import { $, browser, expect } from "@wdio/globals";
import { beforeEach, describe, it } from "mocha";

describe("YAML apply review", () => {
	beforeEach(async () => {
		await browser.url("/");
		await browser.execute(() => localStorage.clear());
		await browser.refresh();
		await $("#workspace-name").setValue("Fast Mock Lab");
		const create = await $("button=Create workspace");
		await create.waitForEnabled();
		await create.click();
	});

	it("does not enable Apply for a draft edited while its dry run was in flight", async () => {
		await $("button=Resources").click();
		await $('input[aria-label="Search resources"]').setValue("payments-api");
		await $('button[aria-label="Open resource payments-api"]').click();
		await $('[role="tab"]=YAML').click();
		await $("button=Edit YAML").click();
		const editor = await $('.cm-content[contenteditable="true"]');
		await editor.waitForDisplayed();

		// Mock commands resolve after a delay, so this edit lands while the dry run is pending.
		await browser.execute((content: HTMLElement) => {
			const dryRun = [...document.querySelectorAll("button")].find((button) => button.textContent?.trim() === "Dry run");
			dryRun?.click();
			content.focus();
			const selection = getSelection();
			selection?.selectAllChildren(content);
			selection?.collapseToEnd();
			document.execCommand("insertText", false, "\n# unreviewed edit");
		}, editor);
		await $("button=Dry run").waitForEnabled();
		await expect(editor).toHaveText(expect.stringContaining("unreviewed edit"));
		await expect($("body")).not.toHaveText(expect.stringContaining("Dry-run diff"));
		await expect($("button=Apply")).toBeDisabled();

		await $("button=Dry run").click();
		await expect($("button=Apply")).toBeEnabled();
	});
});
