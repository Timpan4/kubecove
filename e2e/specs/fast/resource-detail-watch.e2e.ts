import { writeFile } from "node:fs/promises";
import { join } from "node:path";
import { browser, expect } from "@wdio/globals";
import { describe, it } from "mocha";

describe("resource detail watch refresh", () => {
	it("reads again after a watch joins a pre-change details or events read", async () => {
		await browser.url("/");
		const result = await browser.execute(async () => {
			const fixture = "/e2e/fixtures/resource-detail-watch-regression.ts";
			const { resourceDetailWatchRegression } = await import(/* @vite-ignore */ fixture);
			return resourceDetailWatchRegression();
		});
		expect(result).toEqual({ detailReads: 2, eventReads: 2, details: "new", events: [] });
		const artifacts = process.env.KUBECOVE_E2E_ARTIFACTS;
		if (!artifacts) throw new Error("Artifact directory is required");
		await writeFile(join(artifacts, "resource-detail-watch.json"), JSON.stringify(result, null, 2));
	});
	it("does not repeat details refreshes for a disabled fetching Argo inspector", async () => {
		await browser.url("/");
		const result = await browser.execute(async () => {
			const fixture = "/e2e/fixtures/resource-detail-watch-regression.ts";
			const { resourceDetailWatchRegression } = await import(/* @vite-ignore */ fixture);
			return resourceDetailWatchRegression(true);
		});
		expect(result).toEqual({ detailReads: 2, eventReads: 2, details: "new", events: [] });
		const artifacts = process.env.KUBECOVE_E2E_ARTIFACTS;
		if (!artifacts) throw new Error("Artifact directory is required");
		await writeFile(join(artifacts, "resource-detail-disabled-argo-watch.json"), JSON.stringify(result, null, 2));
	});
});
