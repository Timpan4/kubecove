import { writeFile } from "node:fs/promises";
import { join } from "node:path";
import { $, $$, browser, expect } from "@wdio/globals";
import { beforeEach, describe, it } from "mocha";

declare global {
	interface Window {
		copiedIncidentName?: string;
	}
}

describe("Incident Cockpit", () => {
	beforeEach(async () => {
		await browser.url("/");
		await browser.execute(() => localStorage.clear());
		await browser.refresh();
		await $("#workspace-name").setValue("Fast Mock Lab");
		const create = await $("button=Create workspace");
		await create.waitForEnabled();
		await create.click();
	});

	it("reflows compact content without nested scroll panes", async () => {
		const originalSize = await browser.getWindowSize();
		const longResourceName = "opensearch-dashboards-85b6c79f6d-f9x7m";

		try {
			await $('button[aria-label="Open workspace navigation"]').click();
			const navigation = await $('[data-slot="sheet-content"]');
			await navigation.$('[role="treeitem"]*=Incidents').click();
			const layout = await $("[data-incident-layout]");
			const resource = await $(`button*=${longResourceName}`);
			const copy = await $(`button[aria-label="Copy resource name ${longResourceName}"]`);

			for (const [width, height, expectedColumns, expectedOverflow] of [
				[1026, 752, 1, "visible"],
				[1440, 900, 2, "auto"],
			] as const) {
				await browser.sendCommandAndGetResult("Emulation.setDeviceMetricsOverride", {
					width,
					height,
					deviceScaleFactor: 1,
					mobile: false,
				});
				await expect(layout).toBeDisplayed();
				await expect(resource).toBeDisplayed();

				const reflow = await browser.execute((resourceName: string) => {
					const layoutElement = document.querySelector<HTMLElement>("[data-incident-layout]");
					const queueElement = document.querySelector<HTMLElement>("[data-incident-queue-scroll]");
					const guideElement = document.querySelector<HTMLElement>("[data-incident-guide]");
					const resourceButton = Array.from(document.querySelectorAll<HTMLElement>("button")).find(
						(element) => element.textContent?.includes(resourceName),
					);
					if (!layoutElement || !queueElement || !guideElement || !resourceButton) return null;
					const resourceCard = resourceButton.parentElement;
					return {
						columns: getComputedStyle(layoutElement).gridTemplateColumns.split(" ").length,
						guideOverflow: getComputedStyle(guideElement).overflowY,
						queueOverflow: getComputedStyle(queueElement).overflowY,
						resourceFitsCard: (resourceCard?.scrollWidth ?? 1) <= (resourceCard?.clientWidth ?? 0),
						resourceNameVisible: resourceButton.textContent?.includes("Pod/opensearch-dashboards-85b6c79f6d-f9x7m") ?? false,
						statusVisible: resourceButton.textContent?.includes("Needs attention") ?? false,
					};
				}, longResourceName);

				expect(reflow).toEqual({
					columns: expectedColumns,
					guideOverflow: expectedOverflow,
					queueOverflow: expectedOverflow,
					resourceFitsCard: true,
					resourceNameVisible: true,
					statusVisible: true,
				});
			}

			await browser.sendCommandAndGetResult("Emulation.setDeviceMetricsOverride", {
				width: 1026,
				height: 752,
				deviceScaleFactor: 1,
				mobile: false,
			});
			await browser.execute(() => {
				Object.defineProperty(navigator, "clipboard", {
					configurable: true,
					value: {
						writeText: async (text: string) => {
							window.copiedIncidentName = text;
						},
					},
				});
			});
			await copy.click();
			await expect(copy).toHaveText(expect.stringContaining("Copied"));
			expect(
				await browser.execute(
					() => window.copiedIncidentName,
				),
			).toBe(longResourceName);

			await resource.click();
			const keyboardStart = await $("button=Show context");
			const remediation = await $("[data-incident-remediation]");
			await remediation.waitForDisplayed();
			const started = await browser.execute((element: HTMLElement) => {
				const outerScroller = element.closest('[data-slot="scroll-area"]');
				element.focus();
				if (outerScroller instanceof HTMLElement) outerScroller.scrollTop = 0;
				return document.activeElement === element;
			}, keyboardStart);
			expect(started).toBe(true);

			await browser.keys("Tab");
			const investigationStep = await $("[data-incident-investigation-step]");
			expect(
				await browser.execute(
					(element: HTMLElement) => document.activeElement === element && element.matches(":focus-visible"),
					investigationStep,
				),
			).toBe(true);

			const investigationStepCount = await $$('[data-incident-investigation-step]').length;
			for (let index = 0; index < investigationStepCount; index++) await browser.keys("Tab");
			const readKeyboardResult = () => browser.execute((element: HTMLElement) => {
				const bounds = element.getBoundingClientRect();
				const outerScroller = element.closest('[data-slot="scroll-area"]');
				return {
					active: document.activeElement === element,
					focusVisible: element.matches(":focus-visible"),
					outerScrolled: outerScroller instanceof HTMLElement && outerScroller.scrollTop > 0,
					visible: bounds.top >= 0 && bounds.bottom <= document.documentElement.clientHeight,
				};
			}, remediation);
			await browser.waitUntil(async () => {
				const result = await readKeyboardResult();
				return result.active && result.focusVisible && result.outerScrolled && result.visible;
			}, { timeoutMsg: "Keyboard focus did not scroll the remediation action fully into view." });
			expect(await readKeyboardResult()).toEqual({ active: true, focusVisible: true, outerScrolled: true, visible: true });

			// Keep the natural Tab path above, then exercise nearest-edge scrolling on settled content.
			await browser.waitUntil(() => browser.execute(() =>
				!document.querySelector("[data-incident-guide]")?.textContent?.includes("Loading live conditions"),
			));
			const investigationSteps = $$("[data-incident-investigation-step]");
			const lastInvestigationStep = await investigationSteps[(await investigationSteps.length) - 1];
			if (!lastInvestigationStep) throw new Error("The incident guide has no investigation step to Tab from.");
			await browser.execute((element: HTMLElement) => element.focus(), lastInvestigationStep);
			expect(await browser.execute((element: HTMLElement) => {
				const scroller = element.closest<HTMLElement>('[data-slot="scroll-area"]');
				if (!scroller) return false;
				const bounds = element.getBoundingClientRect();
				const clipBottom = scroller.getBoundingClientRect().bottom;
				scroller.scrollTop += bounds.top - (clipBottom - bounds.height / 2);
				const positioned = element.getBoundingClientRect();
				return document.activeElement !== element && positioned.top < clipBottom && positioned.bottom > clipBottom;
			}, remediation)).toBe(true);
			await browser.keys("Tab");
			await browser.waitUntil(async () => {
				const result = await readKeyboardResult();
				return result.active && result.focusVisible && result.outerScrolled && result.visible;
			}, { timeoutMsg: "Keyboard focus left the partially visible remediation action clipped." });
			expect(await readKeyboardResult()).toEqual({ active: true, focusVisible: true, outerScrolled: true, visible: true });
		} catch (error) {
			try {
				const geometry = await browser.execute(() => {
					const element = document.querySelector<HTMLElement>("[data-incident-remediation]");
					const scroller = element?.closest<HTMLElement>('[data-slot="scroll-area"]');
					return {
						viewport: { width: innerWidth, height: innerHeight, clientHeight: document.documentElement.clientHeight, visualHeight: visualViewport?.height },
						bounds: element?.getBoundingClientRect().toJSON(),
						active: document.activeElement === element,
						focusVisible: element?.matches(":focus-visible"),
						scrollTop: scroller?.scrollTop,
						scrollerBounds: scroller?.getBoundingClientRect().toJSON(),
						fonts: document.fonts.status,
					};
				});
				console.error("Incident keyboard geometry", geometry);
				const artifacts = process.env.KUBECOVE_E2E_ARTIFACTS;
				if (artifacts) {
					await writeFile(join(artifacts, "incident-focus-failure.json"), JSON.stringify(geometry, null, 2));
					await browser.saveScreenshot(join(artifacts, "incident-before-viewport-reset.png"));
				}
			} catch (diagnosticError) {
				console.error("Could not retain Incident failure screenshot", diagnosticError);
			}
			throw error;
		} finally {
			await browser.sendCommandAndGetResult("Emulation.clearDeviceMetricsOverride", {});
			await browser.setWindowSize(originalSize.width, originalSize.height);
		}
	});
});
