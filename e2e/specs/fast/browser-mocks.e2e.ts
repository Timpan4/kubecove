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

	it("keeps RBAC summary actions visible at compact and wide sizes", async () => {
		const originalSize = await browser.getWindowSize();

		try {
			await $('button[aria-label="Open workspace navigation"]').click();
			const navigation = await $('[data-slot="sheet-content"]');
			await navigation.$('[role="treeitem"]*=RBAC').click();
			const refresh = await $("button=Refresh");
			const investigate = await $("button=Investigate identity");

			for (const [width, height] of [
				[1026, 752],
				[1440, 900],
			] as const) {
				await browser.sendCommandAndGetResult("Emulation.setDeviceMetricsOverride", {
					width,
					height,
					deviceScaleFactor: 1,
					mobile: false,
				});
				await expect(refresh).toBeDisplayed();
				await expect(investigate).toBeDisplayed();

				const layout = await browser.execute(
					(refreshButton: HTMLElement, investigateButton: HTMLElement) => {
						const actions = investigateButton.parentElement;
						const summary = actions?.parentElement;
						const summaryBounds = summary?.getBoundingClientRect();
						const viewport = {
							width: document.documentElement.clientWidth,
							height: document.documentElement.clientHeight,
						};
						const insideViewport = (element: HTMLElement) => {
							const bounds = element.getBoundingClientRect();
							return bounds.left >= 0 && bounds.right <= viewport.width && bounds.top >= 0 && bounds.bottom <= viewport.height;
						};
						const metricCells = ["Identities", "Roles", "Bindings", "High", "Unknown"].map((label) =>
							Array.from(summary?.querySelectorAll("p") ?? []).find((element) => element.textContent?.trim() === label)?.parentElement,
						);
						const investigateBounds = investigateButton.getBoundingClientRect();
						return {
							metricsVisible: metricCells.every(
								(cell) =>
									cell &&
									insideViewport(cell) &&
									cell.getBoundingClientRect().left >= (summaryBounds?.left ?? 1) &&
									cell.getBoundingClientRect().right <= (summaryBounds?.right ?? 0),
							),
							refreshInsideViewport: insideViewport(refreshButton),
							investigateInsideViewport: insideViewport(investigateButton),
							summaryContainsActions: investigateBounds.right <= (summaryBounds?.right ?? 0),
							summaryHasNoHorizontalOverflow: (summary?.scrollWidth ?? 1) <= (summary?.clientWidth ?? 0),
						};
					},
					refresh,
					investigate,
				);

				expect(layout).toEqual({
					metricsVisible: true,
					refreshInsideViewport: true,
					investigateInsideViewport: true,
					summaryContainsActions: true,
					summaryHasNoHorizontalOverflow: true,
				});

				for (const action of [refresh, investigate]) {
					await browser.keys("Tab");
					const focus = await browser.execute((element: HTMLElement) => {
						element.focus();
						return {
							active: document.activeElement === element,
							focusVisible: element.matches(":focus-visible"),
							boxShadow: getComputedStyle(element).boxShadow,
						};
					}, action);
					expect(focus.active).toBe(true);
					expect(focus.focusVisible).toBe(true);
					expect(focus.boxShadow).not.toBe("none");
				}
			}
		} finally {
			await browser.sendCommandAndGetResult("Emulation.clearDeviceMetricsOverride", {});
			await browser.setWindowSize(originalSize.width, originalSize.height);
		}
	});

	it("reflows the Incident Cockpit at compact size without nested scroll panes", async () => {
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
					},
					longResourceName,
				);

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
							(window as Window & { copiedIncidentName?: string }).copiedIncidentName = text;
						},
					},
				});
			});
			await copy.click();
			await expect(copy).toHaveText(expect.stringContaining("Copied"));
			expect(
				await browser.execute(
					() => (window as Window & { copiedIncidentName?: string }).copiedIncidentName,
				),
			).toBe(longResourceName);

			await resource.click();
			const keyboardStart = await $("button=Show context");
			const remediation = await $("[data-incident-remediation]");
			await remediation.waitForDisplayed();
			const started = await browser.execute((element: HTMLElement) => {
				const outerScroller = element.closest('[data-slot="scroll-area"]') as HTMLElement | null;
				element.focus();
				if (outerScroller) outerScroller.scrollTop = 0;
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
			const keyboardResult = await browser.execute((element: HTMLElement) => {
				const bounds = element.getBoundingClientRect();
				const outerScroller = element.closest('[data-slot="scroll-area"]') as HTMLElement | null;
				return {
					active: document.activeElement === element,
					focusVisible: element.matches(":focus-visible"),
					outerScrolled: (outerScroller?.scrollTop ?? 0) > 0,
					visible: bounds.top >= 0 && bounds.bottom <= document.documentElement.clientHeight,
				};
			}, remediation);
			expect(keyboardResult).toEqual({ active: true, focusVisible: true, outerScrolled: true, visible: true });
		} finally {
			await browser.sendCommandAndGetResult("Emulation.clearDeviceMetricsOverride", {});
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
