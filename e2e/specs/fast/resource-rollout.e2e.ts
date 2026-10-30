import { writeFile } from "node:fs/promises";
import { join } from "node:path";
import { browser, expect } from "@wdio/globals";
import { before, describe, it } from "mocha";

describe("rollout progress and realtime recovery", () => {
	before(async () => {
		// Match the user's existing table screenshot.
		const frame = await browser.execute(() => ({ width: window.outerWidth - window.innerWidth, height: window.outerHeight - window.innerHeight }));
		await browser.setWindowSize(1350 + frame.width, 868 + frame.height);
	});
	for (const mode of ["partial", "polling"] as const) {
		it(`explains periodic refresh with ${mode} watch coverage`, async () => {
			const artifacts = process.env.KUBECOVE_E2E_ARTIFACTS;
			if (!artifacts) throw new Error("Artifact directory is required");
			await browser.url("/");
			await browser.execute(async () => {
				const fixture = "/e2e/fixtures/resource-rollout-regression.ts";
				await (await import(/* @vite-ignore */ fixture)).mountResourceRollout();
			});
			const harness = browser.$("[data-rollout-harness]");
			try {
				await expect(harness.$$("canvas[data-resource-progress='active']")).toBeElementsArrayOfSize(6);
				await browser.execute(async (coverage) => {
					const fixture = "/e2e/fixtures/resource-rollout-regression.ts";
					(await import(/* @vite-ignore */ fixture)).limitResourceRolloutWatches(coverage);
				}, mode);
				await expect(harness).toHaveText(expect.stringContaining(mode === "partial" ? "Live updates are limited." : "Live updates are unavailable."));
				await expect(harness).toHaveText(expect.stringContaining("refresh periodically. Activity indicators are paused."));
				await expect(harness).not.toHaveText(expect.stringContaining("until live updates recover"));
				await expect(harness.$$("canvas[data-resource-progress='active']")).toBeElementsArrayOfSize(0);
				await expect(harness.$$("canvas[data-resource-progress='paused']")).toBeElementsArrayOfSize(6);
				await browser.saveScreenshot(join(artifacts, `resource-rollout-${mode}.png`));
				await writeFile(join(artifacts, `resource-rollout-${mode}.json`), JSON.stringify({ mode, periodicRefreshExplained: true, activityPaused: true }, null, 2));
			} finally {
				await browser.execute(async () => {
					const fixture = "/e2e/fixtures/resource-rollout-regression.ts";
					await (await import(/* @vite-ignore */ fixture)).stopResourceRollout();
				});
			}
		});
	}
	it("keeps lifecycle beside names, replaces deleted pods, and recovers a failed final refresh", async () => {
		const artifacts = process.env.KUBECOVE_E2E_ARTIFACTS;
		if (!artifacts) throw new Error("Artifact directory is required");
		await browser.url("/");
		const setup = await browser.execute(async () => {
			const fixture = "/e2e/fixtures/resource-rollout-regression.ts";
			return (await import(/* @vite-ignore */ fixture)).mountResourceRollout();
		});
		const harness = browser.$("[data-rollout-harness]");
		try {
			await expect(harness.$("table")).toExist();
			const columns = await harness.$$("thead th").map((header) => header.getText());
			expect(columns.slice(0, 4)).toEqual(["Name", "Status", "Namespace", "Kind"]);
			await expect(harness.$("tbody")).toHaveText(expect.stringContaining("Terminating"));
			await expect(harness.$("tbody")).toHaveText(expect.stringContaining("Rolling out"));
			await expect(harness.$("tbody")).not.toHaveText(expect.stringContaining("layercove-new-pod"));
			await browser.execute(async () => {
				const fixture = "/e2e/fixtures/resource-rollout-regression.ts";
				(await import(/* @vite-ignore */ fixture)).addResourceRolloutPod();
			});
			await expect(harness.$("tbody")).toHaveText(expect.stringContaining("layercove-new-pod"));
			await expect(harness.$$(".resource-topology-node")).toBeElementsArrayOfSize(6);
			await expect(harness.$$(".resource-topology-node canvas[data-resource-progress='active']")).toBeElementsArrayOfSize(4);
			await expect(harness.$$(".resource-topology-node.resource-topology-stoplight-info")).toBeElementsArrayOfSize(4);
			await browser.execute(() => {
				const trigger = Array.from(document.querySelectorAll<HTMLButtonElement>("[data-rollout-harness] table button")).find((button) => button.textContent?.trim() === "Rolling out");
				trigger?.focus();
			});
			await expect(browser.$("[role='tooltip']")).toHaveText(expect.stringContaining("Kubernetes"));
			await expect(browser.$("[role='tooltip']")).toHaveText(expect.stringContaining("Rolling out"));
			await expect(browser.$$("[role='tooltip'] summary, [role='tooltip'] button, [role='tooltip'] a")).toBeElementsArrayOfSize(0);
			await browser.execute(async () => {
				const tooltip = document.querySelector<HTMLElement>("[role='tooltip']");
				if (!tooltip) throw new Error("Health evidence tooltip is missing");
				await Promise.all(tooltip.getAnimations().map((animation) => animation.finished));
				const bounds = tooltip.getBoundingClientRect();
				if (!tooltip.contains(document.elementFromPoint(bounds.x + bounds.width / 2, bounds.y + bounds.height / 2))) throw new Error("Health evidence tooltip is obscured");
			});
			await browser.saveScreenshot(join(artifacts, "resource-rollout-health-tooltip.png"));
			await browser.keys("Escape");
			const activity = await browser.execute(async () => {
				const canvas = document.querySelector<HTMLCanvasElement>("[data-rollout-harness] table canvas");
				if (!canvas) throw new Error("Progress indicator is missing");
				const before = canvas.toDataURL();
				await new Promise(requestAnimationFrame);
				await new Promise(requestAnimationFrame);
				return { painted: canvas.getContext("2d")?.getImageData(0, 0, canvas.width, canvas.height).data.some((value, index) => index % 4 === 3 && value > 0), animates: before !== canvas.toDataURL(), color: getComputedStyle(canvas).color };
			});
			expect(activity.painted).toBe(true);
			expect(activity.animates).toBe(true);
			await browser.saveScreenshot(join(artifacts, "resource-rollout-progress.png"));
			await browser.execute(async () => {
				const fixture = "/e2e/fixtures/resource-rollout-regression.ts";
				(await import(/* @vite-ignore */ fixture)).failResourceRollout();
			});
			await expect(harness).toHaveText(expect.stringContaining("Showing last observed state"));
			await expect(harness.$$("canvas[data-resource-progress='paused']")).toBeElementsArrayOfSize(8);
			await harness.$("[data-refresh-error] > summary").click();
			await expect(harness).toHaveText(expect.stringContaining("KubeCove could not reach the Kubernetes API"));
			await harness.$("[data-refresh-error] > summary").click();
			await browser.saveScreenshot(join(artifacts, "resource-rollout-stale.png"));
			await browser.execute(async () => {
				const fixture = "/e2e/fixtures/resource-rollout-regression.ts";
				(await import(/* @vite-ignore */ fixture)).completeResourceRollout();
			});
			await browser.waitUntil(async () => (await harness.getText()).includes("Showing last observed state") === false, { timeout: setup.recoveryInterval + browser.options.waitforTimeout });
			await expect(harness.$("tbody")).not.toHaveText(expect.stringContaining("layercove-old-pod"));
			await expect(harness.$("tbody")).toHaveText(expect.stringContaining("layercove-new-pod"));
			await expect(harness.$(".svelte-flow")).not.toHaveText(expect.stringContaining("layercove-old-pod"));
			await expect(harness.$$("canvas[data-resource-progress]")).toBeElementsArrayOfSize(0);
			await browser.saveScreenshot(join(artifacts, "resource-rollout-complete.png"));
			await writeFile(join(artifacts, "resource-rollout.json"), JSON.stringify({ statusBesideName: true, lifecycleVisible: true, activity, addedPodAppears: true, staleSnapshotVisible: true, recoveredWithoutAnotherEvent: true, deletedPodRemoved: true }, null, 2));
		} catch (error) {
			await browser.saveScreenshot(join(artifacts, "resource-rollout-failure.png"));
			await writeFile(join(artifacts, "resource-rollout-failure.html"), await harness.getHTML());
			throw error;
		} finally {
			await browser.execute(async () => {
				const fixture = "/e2e/fixtures/resource-rollout-regression.ts";
				await (await import(/* @vite-ignore */ fixture)).stopResourceRollout();
			});
		}
	});
	it("pauses placeholder activity while a newly connected namespace is still loading", async () => {
		const artifacts = process.env.KUBECOVE_E2E_ARTIFACTS;
		if (!artifacts) throw new Error("Artifact directory is required");
		await browser.url("/");
		await browser.execute(async () => {
			const fixture = "/e2e/fixtures/resource-rollout-regression.ts";
			await (await import(/* @vite-ignore */ fixture)).mountResourceRollout();
		});
		const harness = browser.$("[data-rollout-harness]");
		try {
			await expect(harness.$$("canvas[data-resource-progress='active']")).toBeElementsArrayOfSize(6);
			await browser.execute(async () => {
				const fixture = "/e2e/fixtures/resource-rollout-regression.ts";
				(await import(/* @vite-ignore */ fixture)).switchResourceRolloutScope();
			});
			await expect(harness).toHaveText(expect.stringContaining("Showing last observed state"));
			await expect(harness.$$("canvas[data-resource-progress='active']")).toBeElementsArrayOfSize(0);
			await expect(harness.$$("canvas[data-resource-progress='paused']")).toBeElementsArrayOfSize(6);
			await browser.saveScreenshot(join(artifacts, "resource-rollout-placeholder.png"));
			await browser.execute(async () => {
				const fixture = "/e2e/fixtures/resource-rollout-regression.ts";
				(await import(/* @vite-ignore */ fixture)).finishResourceRolloutReads();
			});
			await expect(harness.$("tbody")).toHaveText(expect.stringContaining("rollout-next"));
			await expect(harness.$$("canvas[data-resource-progress='active']")).toBeElementsArrayOfSize(6);
			await expect(harness).not.toHaveText(expect.stringContaining("Showing last observed state"));
			await writeFile(join(artifacts, "resource-rollout-placeholder.json"), JSON.stringify({ placeholderActivityPaused: true, resumesForNewNamespace: true }, null, 2));
		} finally {
			await browser.execute(async () => {
				const fixture = "/e2e/fixtures/resource-rollout-regression.ts";
				await (await import(/* @vite-ignore */ fixture)).stopResourceRollout();
			});
		}
	});
	it("keeps cached activity paused until a reconnect catch-up read completes", async () => {
		const artifacts = process.env.KUBECOVE_E2E_ARTIFACTS;
		if (!artifacts) throw new Error("Artifact directory is required");
		await browser.url("/");
		await browser.execute(async () => {
			const fixture = "/e2e/fixtures/resource-rollout-regression.ts";
			await (await import(/* @vite-ignore */ fixture)).mountResourceRollout();
		});
		const harness = browser.$("[data-rollout-harness]");
		try {
			await expect(harness.$$("canvas[data-resource-progress='active']")).toBeElementsArrayOfSize(6);
			await browser.execute(async () => {
				const fixture = "/e2e/fixtures/resource-rollout-regression.ts";
				(await import(/* @vite-ignore */ fixture)).reconnectResourceRollout();
			});
			await expect(harness).toHaveText(expect.stringContaining("Catching up"));
			await expect(harness).toHaveText(expect.stringContaining("Showing last observed state"));
			await expect(harness.$$("canvas[data-resource-progress='active']")).toBeElementsArrayOfSize(0);
			await expect(harness.$$("canvas[data-resource-progress='paused']")).toBeElementsArrayOfSize(6);
			await browser.saveScreenshot(join(artifacts, "resource-rollout-reconnect-pending.png"));
			await browser.execute(async () => {
				const fixture = "/e2e/fixtures/resource-rollout-regression.ts";
				(await import(/* @vite-ignore */ fixture)).finishResourceRolloutReads();
			});
			await expect(harness.$$("canvas[data-resource-progress='active']")).toBeElementsArrayOfSize(6);
			await expect(harness).not.toHaveText(expect.stringContaining("Showing last observed state"));
			await writeFile(join(artifacts, "resource-rollout-reconnect-pending.json"), JSON.stringify({ pausedUntilCatchUp: true, resumesAfterCatchUp: true }, null, 2));
		} finally {
			await browser.execute(async () => {
				const fixture = "/e2e/fixtures/resource-rollout-regression.ts";
				await (await import(/* @vite-ignore */ fixture)).stopResourceRollout();
			});
		}
	});
});
